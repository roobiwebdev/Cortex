/**
 * SOURCE OF TRUTH KEYWORDS: SITE_URL, DOWNLOADS, NOTARIZED, WINDOWS_SIGNED, VERSION, MIN_MACOS, ARCH, REQUIREMENTS, FEATURES, site metadata
 * WHAT:  Every value the landing page states about the product or links to: name, version,
 *        system requirements, the download table, title, description, the release flags and
 *        which features each platform has.
 * WHY:   One place to update for a release or a new domain, and one place to check that each
 *        claim on the page still matches the app.
 * WHERE: routes/+page.svelte, routes/download/+server.ts, lib/platform.ts, the demo and the OG
 *        image text.
 */

// The site's domain. Change static/robots.txt and static/sitemap.xml with it; the canonical,
// Open Graph and JSON-LD tags all use this value.
export const SITE_URL = "https://cortex.roobi.dev";

// From apps/desktop/src-tauri/tauri.conf.json ("version"). Bump with each release.
export const VERSION = "0.1.0";

// `lipo -archs /Applications/Cortex.app/Contents/MacOS/cortex-desktop` prints only arm64.
// Change this to 'Apple Silicon and Intel' once the release build is universal.
export const ARCH = "Apple Silicon";

// tauri.conf.json bundle.macOS.minimumSystemVersion and .cargo/config.toml MACOSX_DEPLOYMENT_TARGET.
export const MIN_MACOS = "15";

/**
 * The app isn't notarized yet, so Gatekeeper blocks the first open. While this is false the
 * install steps explain how to open it anyway. Set it to true once releases are notarized.
 */
export const NOTARIZED = false;

/**
 * The Windows installers aren't code-signed yet, so SmartScreen warns on the first run. While
 * this is false the install steps explain how to run it anyway. Set it to true once they are.
 */
export const WINDOWS_SIGNED = false;

/** Windows and Linux builds are new and untested on many PCs, so the page labels them Beta. */
export const BETA = { windows: true, linux: true } as const;

/** Releases are published on this public repo; every asset name below is stable across versions. */
export const RELEASES_URL = "https://github.com/roobiwebdev/cortex-releases/releases";

export type DownloadOs = "mac" | "windows" | "linux";
export type DownloadArch = "universal" | "x86_64" | "aarch64";
export type DownloadFormat = "dmg" | "exe" | "msi" | "appimage" | "deb" | "rpm";

export interface Download {
  /** The asset's file name on GitHub Releases. */
  file: string;
  url: string;
  /** Short format name for buttons and links, e.g. ".deb". */
  label: string;
}

function asset(file: string, label: string): Download {
  return { file, url: `${RELEASES_URL}/latest/download/${file}`, label };
}

/**
 * Every installer, by OS, then architecture, then format. The macOS DMG is one file for every
 * Mac the build supports (see ARCH). Windows has no arm64 build; Windows on Arm runs the x64 one.
 */
export const DOWNLOADS = {
  mac: {
    universal: { dmg: asset("Cortex-macOS.dmg", ".dmg") },
  },
  windows: {
    x86_64: {
      exe: asset("Cortex-Windows-x64-Setup.exe", "Installer (.exe)"),
      msi: asset("Cortex-Windows-x64.msi", ".msi"),
    },
  },
  linux: {
    x86_64: {
      appimage: asset("Cortex-Linux-x86_64.AppImage", "AppImage"),
      deb: asset("cortex-linux-amd64.deb", ".deb"),
      rpm: asset("cortex-linux-x86_64.rpm", ".rpm"),
    },
    aarch64: {
      appimage: asset("Cortex-Linux-aarch64.AppImage", "AppImage"),
      deb: asset("cortex-linux-arm64.deb", ".deb"),
      rpm: asset("cortex-linux-aarch64.rpm", ".rpm"),
    },
  },
} as const satisfies Record<DownloadOs, Partial<Record<DownloadArch, Partial<Record<DownloadFormat, Download>>>>>;

export type LinuxArch = keyof typeof DOWNLOADS.linux;

/** The Mac DMG: the fallback for `/download` and the prerendered hero button. */
export const DOWNLOAD_URL = DOWNLOADS.mac.universal.dmg.url;

/** Shown as "arm64" or "x86_64" next to Linux downloads. */
export const LINUX_ARCH_LABEL: Record<LinuxArch, string> = { x86_64: "x86_64", aarch64: "arm64" };

/**
 * Oldest supported systems, from the build configs:
 * - Windows: Windows.Graphics.Capture needs 1903; keeping Cortex's own windows out of a
 *   recording needs 2004 (build 19041). Only an x64 build exists. tauri.windows.conf.json
 *   installs WebView2 through the bootstrapper when it is missing.
 * - Linux: Tauri 2 needs WebKitGTK 4.1; screen capture is the xdg-desktop-portal ScreenCast
 *   picker over PipeWire. TODO(release): the Linux CI job builds on ubuntu-24.04, so the
 *   binaries need glibc 2.39; if the release pipeline builds on 22.04, change LINUX_DISTROS.
 */
export const REQUIREMENTS: Record<DownloadOs, readonly string[]> = {
  mac: [`macOS ${MIN_MACOS} Sequoia or later`, ARCH],
  windows: ["Windows 10 (version 2004) or Windows 11, 64-bit", "WebView2 (the installer adds it if missing)"],
  linux: [
    "x86_64 or arm64",
    "PipeWire and xdg-desktop-portal for screen sharing",
    "WebKitGTK 4.1 and GStreamer (the .deb and .rpm install them)",
  ],
};
export const LINUX_DISTROS = "Ubuntu 24.04, Fedora 40, Debian 13 or newer";

/**
 * What each platform can do, from each engine's PlatformCapabilities
 * (rec-engine-macos `mac_capabilities`, rec-engine-windows `caps.rs`, rec-engine-linux `caps.rs`)
 * and docs/engineering/platforms.md. `true` = yes, `false` = not yet, a string = the answer as
 * shown ("No" where the platform itself rules it out, or yes with a limit).
 */
export const FEATURES: ReadonlyArray<{
  name: string;
  mac: boolean | string;
  windows: boolean | string;
  linux: boolean | string;
}> = [
  { name: "Record a display, a window or an area", mac: true, windows: true, linux: "Picked in the system dialog" },
  { name: "Every window of one app", mac: true, windows: false, linux: "No (Wayland)" },
  { name: "Camera overlay, mic and system audio", mac: true, windows: true, linux: true },
  { name: "Camera on its own track", mac: true, windows: false, linux: true },
  { name: "Start and stop from any app", mac: true, windows: true, linux: "If the desktop supports it" },
  { name: "Keeps its own windows out of the video", mac: true, windows: "Windows 10 2004+", linux: "No (Wayland)" },
  { name: "Live preview", mac: true, windows: false, linux: false },
  { name: "Quick trim", mac: true, windows: false, linux: false },
  { name: "Replay buffer", mac: true, windows: false, linux: false },
  { name: "Go live", mac: true, windows: false, linux: false },
  { name: "ProRes", mac: true, windows: "No", linux: "No" },
];

export const NAME = "Cortex";

/** Who makes Cortex: the footer line and the JSON-LD author. */
export const AUTHOR = "Roobi";
export const AUTHOR_URL = "https://roobi.dev";
/** The public repo the releases come from; the footer links to it. */
export const GITHUB_URL = "https://github.com/roobiwebdev/cortex-releases";
/** Open-source notices for the code Cortex is built on (static/licenses.txt), linked in the footer. */
export const LICENSES_PATH = "/licenses.txt";

export const TITLE = `${NAME}: a screen and camera recorder for Mac, Windows and Linux`;

export const DESCRIPTION =
  "Cortex records your screen, system audio, mic and camera into one file on Mac, Windows and Linux. On the Mac it adds true 4K, Quick trim, a replay buffer and Go live.";

// Describes og.jpg as rendered (scripts/og).
export const OG_IMAGE_ALT =
  'The Cortex icon and name above "One key. One file." and "A screen and camera recorder for Mac, Windows and Linux.", beside a screenshot of the Cortex window with its sources sidebar, preview and red Record button.';
