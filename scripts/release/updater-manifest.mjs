#!/usr/bin/env node
/**
 * SOURCE OF TRUTH KEYWORDS: latest.json, updater manifest, release assets, platform keys,
 *   minisign verify, signature check, asset names, requireSignedVersion
 * WHAT:  Writes and checks the Tauri updater feed (latest.json) for a Cortex release, and
 *        verifies every updater signature against the public key in tauri.conf.json.
 * WHY:   One place maps the stable release asset names (the contract with the site) to the
 *        updater's platform keys, so the macOS script, assemble.sh and publish.sh can't drift.
 *        `tauri signer` has no verify command, so this checks the minisign signatures itself
 *        (Ed25519 over the BLAKE2b-512 of the file, plus the trusted comment with the version).
 * WHERE: scripts/desktop/release-macos.sh, scripts/release/assemble.sh, scripts/release/publish.sh.
 *
 *   node scripts/release/updater-manifest.mjs write --dir DIR --version V --base-url URL \
 *        [--notes-file FILE] [--darwin-arches aarch64,x86_64] [--require macos,windows-x64,...] \
 *        [--out DIR/latest.json]
 *   node scripts/release/updater-manifest.mjs check --dir DIR [--manifest DIR/latest.json] \
 *        [--version V] [--base-url URL]
 *   node scripts/release/updater-manifest.mjs verify FILE...      (FILE.sig next to each)
 *   node scripts/release/updater-manifest.mjs assets [--group G] [--only files|updater]
 *
 * URLs are <base-url>/<asset>, e.g.
 * https://github.com/roobiwebdev/cortex-releases/releases/download/v0.1.0/Cortex-macOS.app.tar.gz
 */
import { createHash, createPublicKey, verify as edVerify } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const tauriConf = join(root, "apps/desktop/src-tauri/tauri.conf.json");

// The release groups and their files. `updater` lists the platform keys a file serves:
// the updater looks for `{os}-{arch}-{installer}` first (installer = how the running copy was
// installed), then `{os}-{arch}`, so a .deb install gets a .deb update and so on.
export const GROUPS = {
  macos: [
    { file: "Cortex-macOS.dmg" },
    { file: "Cortex-macOS.app.tar.gz", updater: ["darwin-aarch64", "darwin-x86_64"] },
  ],
  "windows-x64": [
    { file: "Cortex-Windows-x64-Setup.exe", updater: ["windows-x86_64", "windows-x86_64-nsis"] },
    { file: "Cortex-Windows-x64.msi", updater: ["windows-x86_64-msi"] },
  ],
  "linux-x64": [
    { file: "Cortex-Linux-x86_64.AppImage", updater: ["linux-x86_64", "linux-x86_64-appimage"] },
    { file: "cortex-linux-amd64.deb", updater: ["linux-x86_64-deb"] },
    { file: "cortex-linux-x86_64.rpm", updater: ["linux-x86_64-rpm"] },
  ],
  "linux-arm64": [
    { file: "Cortex-Linux-aarch64.AppImage", updater: ["linux-aarch64", "linux-aarch64-appimage"] },
    { file: "cortex-linux-arm64.deb", updater: ["linux-aarch64-deb"] },
    { file: "cortex-linux-aarch64.rpm", updater: ["linux-aarch64-rpm"] },
  ],
};

const KNOWN_KEYS = new Set(
  Object.values(GROUPS)
    .flat()
    .flatMap((a) => a.updater ?? []),
);
const SEMVER = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$/;
const RFC3339 = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$/;

function fail(message) {
  console.error(`updater-manifest: ${message}`);
  process.exit(1);
}

function parseArgs(argv) {
  const opts = { _: [] };
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg.startsWith("--")) {
      const value = argv[i + 1];
      if (value === undefined || value.startsWith("--")) fail(`${arg} needs a value`);
      opts[arg.slice(2)] = value;
      i++;
    } else {
      opts._.push(arg);
    }
  }
  return opts;
}

// --- minisign, as written by `tauri signer sign` and `tauri build` -------------------------

function publicKeyFromConfig() {
  const conf = JSON.parse(readFileSync(tauriConf, "utf8"));
  const pubkey = conf?.plugins?.updater?.pubkey;
  if (typeof pubkey !== "string") fail(`no plugins.updater.pubkey in ${tauriConf}`);
  // The config holds base64 of the minisign public key file (comment line + key line).
  const keyLine = Buffer.from(pubkey, "base64").toString("utf8").split("\n")[1]?.trim();
  const raw = Buffer.from(keyLine ?? "", "base64");
  if (raw.length !== 42 || raw.subarray(0, 2).toString() !== "Ed")
    fail("the updater pubkey is not a minisign Ed25519 key");
  return {
    keyId: raw.subarray(2, 10),
    key: createPublicKey({
      key: { kty: "OKP", crv: "Ed25519", x: raw.subarray(10).toString("base64url") },
      format: "jwk",
    }),
  };
}

/** Returns the signed version, or throws with the reason the signature does not verify. */
export function verifySignature(filePath, signatureB64, publicKey) {
  const lines = Buffer.from(signatureB64.trim(), "base64").toString("utf8").split("\n");
  const sig = Buffer.from(lines[1]?.trim() ?? "", "base64");
  const trustedLine = lines[2] ?? "";
  const globalSig = Buffer.from(lines[3]?.trim() ?? "", "base64");
  if (sig.length !== 74 || !trustedLine.startsWith("trusted comment: ") || globalSig.length !== 64) {
    throw new Error("not a minisign signature");
  }
  const algorithm = sig.subarray(0, 2).toString();
  if (!sig.subarray(2, 10).equals(publicKey.keyId)) throw new Error("signed by a different key");
  const data = readFileSync(filePath);
  // "ED" signs the BLAKE2b-512 of the file (what Tauri writes), "Ed" the file itself.
  const message = algorithm === "ED" ? createHash("blake2b512").update(data).digest() : data;
  if (algorithm !== "ED" && algorithm !== "Ed") throw new Error(`unknown algorithm ${algorithm}`);
  if (!edVerify(null, message, publicKey.key, sig.subarray(10))) throw new Error("file signature does not verify");
  const trusted = trustedLine.slice("trusted comment: ".length);
  if (!edVerify(null, Buffer.concat([sig.subarray(10), Buffer.from(trusted)]), publicKey.key, globalSig)) {
    throw new Error("trusted comment signature does not verify");
  }
  const version = trusted.split("\t").find((f) => f.startsWith("version:"));
  return version ? version.slice("version:".length) : null;
}

function verifyFile(filePath, version, publicKey) {
  const sigPath = `${filePath}.sig`;
  if (!existsSync(sigPath)) fail(`${basename(filePath)}: no ${basename(sigPath)}`);
  const signature = readFileSync(sigPath, "utf8").trim();
  let signed;
  try {
    signed = verifySignature(filePath, signature, publicKey);
  } catch (error) {
    fail(`${basename(filePath)}: ${error instanceof Error ? error.message : String(error)}`);
  }
  // requireSignedVersion: the app refuses a signature that does not name the announced version.
  if (signed === null) fail(`${basename(filePath)}: the signature carries no version (sign with --app-version)`);
  if (version && signed !== version) fail(`${basename(filePath)}: signed for ${signed}, not ${version}`);
  return { signature, signed };
}

// --- commands --------------------------------------------------------------------------

function write(opts) {
  const dir = resolve(opts.dir ?? fail("write needs --dir"));
  const version = opts.version ?? fail("write needs --version");
  if (!SEMVER.test(version)) fail(`${version} is not a semver version`);
  const base = (opts["base-url"] ?? fail("write needs --base-url")).replace(/\/+$/, "");
  const darwinArches = (opts["darwin-arches"] ?? "aarch64").split(",").filter(Boolean);
  const required = (opts.require ?? "").split(",").filter(Boolean);
  for (const g of required) if (!GROUPS[g]) fail(`unknown group ${g} (${Object.keys(GROUPS).join(", ")})`);
  const notes = opts["notes-file"] ? readFileSync(opts["notes-file"], "utf8").trim() : `Cortex ${version}`;
  const publicKey = publicKeyFromConfig();

  const platforms = {};
  const included = [];
  for (const [group, groupAssets] of Object.entries(GROUPS)) {
    const present = groupAssets.filter((a) => existsSync(join(dir, a.file)));
    if (present.length === 0) {
      if (required.includes(group)) fail(`required ${group} files are missing from ${dir}`);
      continue;
    }
    if (present.length !== groupAssets.length) {
      const missing = groupAssets.filter((a) => !present.includes(a)).map((a) => a.file);
      fail(`${group} is incomplete, missing ${missing.join(", ")}`);
    }
    included.push(group);
    for (const asset of groupAssets) {
      if (!asset.updater) continue;
      const { signature } = verifyFile(join(dir, asset.file), version, publicKey);
      for (const key of asset.updater) {
        if (key.startsWith("darwin-") && !darwinArches.includes(key.slice("darwin-".length))) continue;
        platforms[key] = { signature, url: `${base}/${asset.file}` };
      }
    }
  }
  if (Object.keys(platforms).length === 0) fail(`no updater bundles in ${dir}`);

  const manifest = {
    version,
    notes,
    pub_date: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
    platforms,
  };
  const out = opts.out ?? join(dir, "latest.json");
  writeFileSync(out, `${JSON.stringify(manifest, null, 2)}\n`);
  console.log(`wrote ${out}: ${included.join(", ")}; ${Object.keys(platforms).length} platform keys`);
}

/** Checks a manifest the way the updater reads it, then every signature against its file. */
function check(opts) {
  const dir = resolve(opts.dir ?? fail("check needs --dir"));
  const path = opts.manifest ?? join(dir, "latest.json");
  let manifest;
  try {
    manifest = JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    fail(`${path}: ${error instanceof Error ? error.message : String(error)}`);
  }
  const problems = [];
  if (typeof manifest !== "object" || manifest === null || Array.isArray(manifest)) fail(`${path}: not an object`);
  // The updater accepts an optional leading "v"; the release scripts never write one.
  if (typeof manifest.version !== "string" || !SEMVER.test(manifest.version)) problems.push("version is not semver");
  if (opts.version && manifest.version !== opts.version)
    problems.push(`version ${manifest.version}, expected ${opts.version}`);
  if (manifest.notes !== undefined && typeof manifest.notes !== "string") problems.push("notes is not a string");
  if (manifest.pub_date !== undefined && (typeof manifest.pub_date !== "string" || !RFC3339.test(manifest.pub_date))) {
    problems.push("pub_date is not RFC 3339");
  }
  const platforms = manifest.platforms;
  if (typeof platforms !== "object" || platforms === null || Object.keys(platforms).length === 0) {
    problems.push("platforms is missing or empty");
  }
  const publicKey = publicKeyFromConfig();
  for (const [key, entry] of Object.entries(platforms ?? {})) {
    if (!KNOWN_KEYS.has(key)) problems.push(`${key}: unknown platform key`);
    if (typeof entry?.url !== "string" || typeof entry?.signature !== "string") {
      problems.push(`${key}: needs string url and signature`);
      continue;
    }
    let url;
    try {
      url = new URL(entry.url);
    } catch {
      problems.push(`${key}: url does not parse`);
      continue;
    }
    // Release builds only accept https feeds and bundles (dangerousInsecureTransportProtocol is off).
    if (url.protocol !== "https:" && !opts["allow-http"]) problems.push(`${key}: url is not https`);
    if (opts["base-url"] && !entry.url.startsWith(`${opts["base-url"].replace(/\/+$/, "")}/`)) {
      problems.push(`${key}: url is not under ${opts["base-url"]}`);
    }
    const file = join(dir, decodeURIComponent(basename(url.pathname)));
    const expected = Object.values(GROUPS)
      .flat()
      .find((a) => a.updater?.includes(key))?.file;
    if (basename(file) !== expected) problems.push(`${key}: points at ${basename(file)}, expected ${expected}`);
    if (!existsSync(file)) {
      problems.push(`${key}: ${basename(file)} is not in ${dir}`);
      continue;
    }
    try {
      const signed = verifySignature(file, entry.signature, publicKey);
      if (signed !== manifest.version)
        problems.push(`${key}: signature names version ${signed}, feed says ${manifest.version}`);
    } catch (error) {
      problems.push(`${key}: ${error instanceof Error ? error.message : String(error)}`);
    }
  }
  if (problems.length > 0) fail(`${path}:\n  ${problems.join("\n  ")}`);
  console.log(`${path}: valid, ${Object.keys(platforms).length} platform keys, every signature verifies`);
  for (const [key, entry] of Object.entries(platforms)) console.log(`  ${key.padEnd(24)} ${entry.url}`);
}

function verifyCommand(opts) {
  if (opts._.length === 0) fail("verify needs at least one file");
  const publicKey = publicKeyFromConfig();
  for (const file of opts._) {
    const { signed } = verifyFile(resolve(file), opts.version, publicKey);
    console.log(`${basename(file)}: signature OK (version ${signed})`);
  }
}

/** Lists release file names: all (with .sig), "files" (no .sig) or "updater" (signed files). */
function assets(opts) {
  const groups = opts.group ? [opts.group] : Object.keys(GROUPS);
  for (const g of groups) {
    for (const a of GROUPS[g] ?? fail(`unknown group ${g}`)) {
      if (opts.only === "updater" && !a.updater) continue;
      console.log(a.file);
      if (a.updater && opts.only === undefined) console.log(`${a.file}.sig`);
    }
  }
}

const [command, ...rest] = process.argv.slice(2);
const opts = parseArgs(rest);
switch (command) {
  case "write":
    write(opts);
    break;
  case "check":
    check(opts);
    break;
  case "verify":
    verifyCommand(opts);
    break;
  case "assets":
    assets(opts);
    break;
  default:
    fail("usage: updater-manifest.mjs write|check|verify|assets (see the header)");
}
