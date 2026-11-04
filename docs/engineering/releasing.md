# Releasing Cortex (macOS, Windows, Linux)

One release is one GitHub release `v<version>` in the public repository **`roobiwebdev/cortex-releases`**. It holds every installer, the updater files with their signatures, `latest.json` (the update feed) and `SHA256SUMS`. The source stays in the private repository **`roobiwebdev/cortex`**, where CI builds the Windows and Linux installers.

```
bump-version.sh ─► tag v<version> ─► release.yml (GitHub)          ─► artifacts, 7 days
                                      windows-x64, linux-x64, linux-arm64
                                                    │ gh run download
release-macos.sh (this Mac, signed) ─► assemble.sh ─┘ signs updater files, writes latest.json
                                            │
                                       publish.sh ─► roobiwebdev/cortex-releases, release v<version>
```

Two secrets never leave the release Mac and are never put in CI: the **updater private key** (`~/.config/cortex/updater.key`, password in the Keychain item `cortex-updater-key`) and the **Cortex Dev Signing** code-signing identity. That is why there is no macOS job in CI, and why CI builds the Windows and Linux installers without updater signatures; `assemble.sh` signs them after downloading. [desktop.md, Keys](desktop.md#keys-and-where-they-live) has the backup rules.

Nothing in this flow uploads anything until you run `publish.sh` and type the version.

## The files of a release

These names are a contract with the site's download links (`.../releases/latest/download/<name>`). They never carry the version. `scripts/release/updater-manifest.mjs` (`GROUPS`) is where they are defined, with the updater keys each file serves.

| Group         | File                                     | Updater keys in `latest.json`             | Built by                               |
| ------------- | ---------------------------------------- | ----------------------------------------- | -------------------------------------- |
| `macos`       | `Cortex-macOS.dmg`                       | (installer only)                          | `release-macos.sh`, universal          |
|               | `Cortex-macOS.app.tar.gz` + `.sig`       | `darwin-aarch64`, `darwin-x86_64`         | same; signed by `tauri build`          |
| `windows-x64` | `Cortex-Windows-x64-Setup.exe` + `.sig`  | `windows-x86_64`, `windows-x86_64-nsis`   | `release.yml`, signed by `assemble.sh` |
|               | `Cortex-Windows-x64.msi` + `.sig`        | `windows-x86_64-msi`                      | same                                   |
| `linux-x64`   | `Cortex-Linux-x86_64.AppImage` + `.sig`  | `linux-x86_64`, `linux-x86_64-appimage`   | same                                   |
|               | `cortex-linux-amd64.deb` + `.sig`        | `linux-x86_64-deb`                        | same                                   |
|               | `cortex-linux-x86_64.rpm` + `.sig`       | `linux-x86_64-rpm`                        | same                                   |
| `linux-arm64` | `Cortex-Linux-aarch64.AppImage` + `.sig` | `linux-aarch64`, `linux-aarch64-appimage` | `release.yml` (arm64 runner) or Docker |
|               | `cortex-linux-arm64.deb` + `.sig`        | `linux-aarch64-deb`                       | same                                   |
|               | `cortex-linux-aarch64.rpm` + `.sig`      | `linux-aarch64-rpm`                       | same                                   |
| (all)         | `latest.json`, `SHA256SUMS`              |                                           | `assemble.sh`                          |

How the updater picks its file (`tauri-plugin-updater` 2.14, `get_urls`): it looks for `{os}-{arch}-{installer}` first, where the installer is how the running copy was installed, then `{os}-{arch}`. So an MSI install updates from the MSI, a `.deb` install from the `.deb` (installed with `pkexec dpkg -i`, which asks for the password), an `.rpm` install from the `.rpm`, and everything else from the NSIS installer or the AppImage. Since Tauri 2 the Windows and Linux updater file is the installer itself, signed; only macOS uses a tarball (`Cortex-macOS.app.tar.gz`). The macOS build is universal, so both `darwin-aarch64` and `darwin-x86_64` point at the same tarball.

Every signature names its version (`tauri signer sign --app-version`, and `tauri build` for macOS) because the app sets `requireSignedVersion`: a feed that pairs a new version number with an older signed file is refused.

## Each release

1. **Bump the version** on the release branch and commit:

   ```sh
   scripts/desktop/bump-version.sh 0.2.0
   git commit -am "Release 0.2.0"
   git push
   ```

   Use plain `MAJOR.MINOR.PATCH`. A pre-release suffix must be numeric (`0.2.0-1`), because the MSI (WiX) build rejects `0.2.0-beta.1`.

2. **Start the CI build**, either by tag or by hand:

   ```sh
   git tag v0.2.0 && git push origin v0.2.0
   # or
   gh workflow run release.yml --repo roobiwebdev/cortex --ref agent/desktop-recorder-4c1e9 -f version=0.2.0
   ```

   The run is titled `Release v0.2.0`. Its first job stops if `tauri.conf.json` says another version.

3. **Wait for CI** (about 30 to 45 minutes; the jobs run the platform's fmt, clippy and tests before building):

   ```sh
   gh run watch --repo roobiwebdev/cortex $(gh run list --repo roobiwebdev/cortex --workflow release.yml --limit 1 --json databaseId --jq '.[0].databaseId')
   ```

   Each job uploads one artifact (`cortex-windows-x64`, `cortex-linux-x64`, `cortex-linux-arm64`) holding its files under the release names.

4. **Assemble** on the release Mac, from a checkout of the same commit (it refuses artifacts built from another commit unless you pass `--allow-commit-mismatch`):

   ```sh
   scripts/release/assemble.sh 0.2.0 --notes-file notes.md
   ```

   It downloads the artifacts of the latest completed `Release v0.2.0` run (`gh run download`), builds the macOS part with `scripts/desktop/release-macos.sh` unless `dist/release/0.2.0/macos/` already holds a build of this version, signs every Windows and Linux updater file with the updater key, writes `latest.json`, checks it the way the updater reads it (every signature verified against the public key in `tauri.conf.json`, every URL https and under the release), and writes `SHA256SUMS`. The result is `dist/release/0.2.0/assets/`. It ends by printing the publish commands.

   Useful options: `--from DIR` takes the Windows/Linux files from a folder instead of GitHub, `--run-id ID` picks a run, `--rebuild-macos` forces the macOS build, `--require-all` refuses a release without the Linux arm64 files (by default they are left out with a warning).

5. **Publish**. Look at `dist/release/0.2.0/assets/` first, then:

   ```sh
   scripts/release/publish.sh 0.2.0 --dry-run   # checks the files and prints the gh commands
   scripts/release/publish.sh 0.2.0             # asks you to type the version, then runs them
   ```

   It re-checks `SHA256SUMS` and `latest.json`, creates `roobiwebdev/cortex-releases` if it does not exist yet, refuses a tag that already has a release, and runs:

   ```sh
   gh release create v0.2.0 --repo roobiwebdev/cortex-releases --title "Cortex 0.2.0" \
     --notes-file dist/release/0.2.0/notes.md --latest dist/release/0.2.0/assets/*
   ```

6. **The site** links to `https://github.com/roobiwebdev/cortex-releases/releases/latest/download/<file>`, so it serves the new installers as soon as the release exists.

7. **Installed apps** read `https://github.com/roobiwebdev/cortex-releases/releases/latest/download/latest.json` (`plugins.updater.endpoints` in `tauri.conf.json`): the quiet daily check, or at once with **Check now** in Settings → General.

**Do not mark a release as a pre-release.** GitHub's `releases/latest` is the newest release that is neither a draft nor a pre-release, so a pre-release would be invisible to both the site and the updater. The beta status is said in the notes and on the site instead. To pull a broken release, publish a fixed higher version; deleting the release makes `latest` fall back to the previous one, but apps that already updated stay on the broken version (the updater never downgrades).

## One-time setup (the user)

1. **Source repository.** From the main checkout, on the branch that becomes the default:

   ```sh
   gh repo create roobiwebdev/cortex --private --description "Cortex desktop recorder"
   git remote add cortex https://github.com/roobiwebdev/cortex.git
   git push cortex agent/desktop-recorder-4c1e9
   gh repo edit roobiwebdev/cortex --default-branch agent/desktop-recorder-4c1e9
   ```

   Pushing `.github/workflows/*` over HTTPS needs a token with the `workflow` scope (`gh auth refresh -s workflow`).

2. **Enable Actions.** New repositories have Actions on. Check Settings → Actions → General: "Allow all actions" (or allow `actions/*`, `pnpm/action-setup` and `Swatinem/rust-cache`), and that workflows may run. `release.yml` must be on the default branch for **Run workflow** (and `gh workflow run`) to find it; tag pushes use the workflow file of the tagged commit.

3. **Linux arm64 runners.** `linux-arm64` runs on GitHub's hosted `ubuntu-24.04-arm` runner. GitHub offers it free for public repositories; for a private repository it depends on the account's plan. If the job sits in "Waiting for a runner", cancel the run, set the repository variable `LINUX_ARM64` to `off` (Settings → Secrets and variables → Actions → Variables, or `gh variable set LINUX_ARM64 --body off --repo roobiwebdev/cortex`), start it again, and build arm64 on the Mac instead:

   ```sh
   scripts/release/build-linux-docker.sh          # stages into dist/release/<version>/linux-arm64/
   ```

   `assemble.sh` picks that folder up when the CI artifact is missing. It needs Docker and the `cortex-linux-dev` image (`scripts/desktop/linux-dev.Dockerfile`), and caches its build in the volumes `cortex-release-linux-target` and `cortex-release-linux-cargo` (`--remove-volumes` deletes them). A failing `linux-arm64` job does not fail the run (`continue-on-error`).

4. **Downloads repository.** `publish.sh` creates it on the first publish. By hand:

   ```sh
   gh repo create roobiwebdev/cortex-releases --public --add-readme --description "Cortex desktop downloads"
   ```

   `--add-readme` gives it a first commit; GitHub cannot tag a release in an empty repository.

5. **Actions minutes.** A private repository's free minutes are limited, and runners are billed with multipliers (Windows 2×, macOS 10×). `release.yml` uses Windows and Linux only. The macOS CI workflow (`desktop.yml`) runs on pushes to `main` and on pull requests that touch desktop code; at 10× it is the expensive one.

CI on the source repository: `desktop-windows.yml` and `desktop-linux.yml` run on pushes to `main` and `agent/desktop-recorder-4c1e9` and on pull requests (fmt, clippy, tests, installers as artifacts). `release.yml` calls the same two workflows, so a release runs exactly the CI checks before it builds.

## What CI builds, and what it doesn't

| Platform          | Where                                                    | Signed how                                                                |
| ----------------- | -------------------------------------------------------- | ------------------------------------------------------------------------- |
| macOS (universal) | this Mac, `scripts/desktop/release-macos.sh`             | Cortex Dev Signing (hardened runtime); updater signature by `tauri build` |
| Windows x64       | `release.yml`, `windows-latest`                          | not code-signed; updater signature by `assemble.sh`                       |
| Linux x86_64      | `release.yml`, `ubuntu-24.04`                            | updater signature by `assemble.sh` (Linux packages are not signed)        |
| Linux aarch64     | `release.yml`, `ubuntu-24.04-arm`, or Docker on this Mac | same                                                                      |

The macOS build needs both Rust targets (`rustup target add x86_64-apple-darwin`; `rust-toolchain.toml` lists them) and about 6 GB in `target/` (each architecture's release build, plus the universal bundle). `--arm64-only` builds Apple Silicon alone. The vendored `third_party/apple-cryptokit-rs` builds its Swift package per architecture (`swift build --triple x86_64-apple-macosx15.0`, native build system), which works without changes.

## Before leaving beta

### Apple notarization (macOS)

Without it, Gatekeeper blocks the first launch and users must allow Cortex in System Settings → Privacy & Security. Nothing in the code changes; [desktop.md, Notarization](desktop.md#notarization-when-the-apple-developer-membership-arrives) has the full steps. In short:

1. Join the Apple Developer Program, create a **Developer ID Application** certificate from a CSR made on this Mac (the key stays in the login keychain).
2. Create an app-specific password or an App Store Connect API key for `notarytool`.
3. Build with the new identity and credentials; `assemble.sh` passes the environment through to `release-macos.sh`:

   ```sh
   CORTEX_SIGNING_IDENTITY="Developer ID Application: <Name> (<TEAMID>)" \
   APPLE_ID=<apple id> APPLE_PASSWORD=<app-specific password> APPLE_TEAM_ID=<TEAMID> \
     scripts/release/assemble.sh 0.3.0 --rebuild-macos --notes-file notes.md
   ```

4. Also set `bundle.macOS.signingIdentity` in `tauri.conf.json` to the new identity, so debug and local builds match.
5. Say in that release's notes that macOS asks for Screen Recording, Camera and Microphone once more: the designated requirement changes from the Cortex Dev Signing certificate to the team ID. Later releases keep the grants. The updater key stays the same.

### Windows code signing (Azure Trusted Signing)

Without it, SmartScreen warns on the first download and run ("Windows protected your PC"). Azure Trusted Signing (Microsoft's signing service, now also sold as Artifact Signing) issues short-lived certificates from a cloud key; there is no certificate file to protect. Signing has to happen while Tauri bundles, so the app `.exe` inside the installer is signed as well as the installer, which means in CI. GitHub's OIDC token logs in to Azure, so no long-lived secret is stored in the repository. The updater signature is still made on the Mac afterwards (it covers the signed bytes).

1. In Azure: create a Trusted Signing account in a supported region, complete identity validation (organization, or individual where offered), and create a **Public Trust** certificate profile. Note the endpoint (e.g. `https://weu.codesigning.azure.net`), account name and profile name.
2. Create an app registration (Microsoft Entra ID) with a **federated credential** for GitHub Actions: organization `roobiwebdev`, repository `cortex`, entity type "Environment", name `release`. (A federated credential matches one exact branch, tag or environment, not a pattern like `v*`, so an environment is the stable choice.) Give the app the role **Trusted Signing Certificate Profile Signer** on the certificate profile.
3. In `roobiwebdev/cortex`, create the environment `release` (Settings → Environments) and the repository variables `AZURE_CLIENT_ID`, `AZURE_TENANT_ID`, `AZURE_SUBSCRIPTION_ID`, `TRUSTED_SIGNING_ENDPOINT`, `TRUSTED_SIGNING_ACCOUNT`, `TRUSTED_SIGNING_PROFILE`. These are identifiers, not secrets.
4. In `desktop-windows.yml`, for release runs only (`if: env.VERSION != ''`): give the job `environment: release` and `permissions: { contents: read, id-token: write }` (and grant `id-token: write` in `release.yml` too), add `azure/login` (pinned by commit) with the three `AZURE_*` variables, install the signer (`cargo install trusted-signing-cli --locked`), and build the installers with

   ```
   pnpm --filter desktop tauri build --bundles nsis,msi --config '{"bundle":{"windows":{"signCommand":"trusted-signing-cli -e <endpoint> -a <account> -c <profile> -d Cortex %1"}}}'
   ```

   Tauri then signs `cortex-desktop.exe`, the NSIS installer and the MSI.

5. Check on a Windows PC: the installer's Properties → Digital Signatures names the validated identity, and `Get-AuthenticodeSignature` reports `Valid`. SmartScreen reputation still builds up over the first downloads.

### Also worth doing

- Run each platform's installer once on a real machine before the first public release: [windows-testing.md](windows-testing.md) and [linux-testing.md](linux-testing.md). CI proves the build and the unit tests, not capture.
- Linux packages are not GPG-signed. If Cortex ever ships through an apt or dnf repository, that repository needs its own signing key.

## Testing the pipeline locally

- `scripts/release/assemble.sh <version> --from <dir>` with a folder of Windows/Linux files (real ones from a CI run, or from `build-linux-docker.sh`) runs every step except the download.
- `node scripts/release/updater-manifest.mjs check --dir dist/release/<version>/assets --version <version>` re-checks a feed; `... verify <file>` checks one file's `.sig`.
- `scripts/release/publish.sh <version> --dry-run` checks the assembled folder and prints the commands without contacting GitHub.
- The updater itself: [desktop.md, Testing the updater locally](desktop.md#testing-the-updater-locally).
