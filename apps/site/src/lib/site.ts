/**
 * SOURCE OF TRUTH KEYWORDS: SITE_URL, DOWNLOAD_URL, NOTARIZED, VERSION, MIN_MACOS, ARCH, WAITLIST_EMAIL, site metadata
 * WHAT:  Every value the landing page states about the product or links to: name, version,
 *        system requirements, URLs, title, description and the release flags.
 * WHY:   One place to update for a release or a new domain, and one place to check that each
 *        claim on the page still matches the app.
 * WHERE: routes/+page.svelte, routes/download/+server.ts, the demo and the OG image text.
 */

// TODO(domain): hosting isn't decided. Update SITE_URL, static/robots.txt and static/sitemap.xml
// together once it is; the canonical, Open Graph and JSON-LD tags all use this value.
export const SITE_URL = "https://cortex.example";

// From apps/desktop/src-tauri/tauri.conf.json ("version"). Bump with each release.
export const VERSION = "0.1.0";

// `lipo -archs /Applications/Cortex.app/Contents/MacOS/cortex-desktop` prints only arm64.
// Change this to 'Apple Silicon and Intel' once the release build is universal.
export const ARCH = "Apple Silicon";

// tauri.conf.json bundle.macOS.minimumSystemVersion and .cargo/config.toml MACOSX_DEPLOYMENT_TARGET.
export const MIN_MACOS = "15";

// TODO(download): where the .dmg will live isn't decided. /download redirects here, so this is the
// only place to change when it is.
export const DOWNLOAD_URL = "https://cortex.example/releases/Cortex.dmg";

/**
 * The app isn't notarized yet, so Gatekeeper blocks the first open. While this is false the
 * install steps explain how to open it anyway. Set it to true once releases are notarized.
 */
export const NOTARIZED = false;

// TODO(waitlist): no list exists yet. When this is set, the Windows and Linux notes link to it as a
// mailto; while it is empty they only say the builds are coming.
export const WAITLIST_EMAIL = "";

export const NAME = "Cortex";

/** Cortex's engine is Rec. The footer credits it, as NOTICE does. */
export const UPSTREAM_NAME = "Rec";
export const UPSTREAM_URL = "https://github.com/michaelshimeles/rec-rs";
export const UPSTREAM_AUTHOR = "Michael Shimeles";

export const TITLE = `${NAME}: a screen and camera recorder for the Mac`;

export const DESCRIPTION =
  "Cortex records your screen, system audio, mic and camera into one file on your Mac, with the camera on its own track, true 4K, Quick trim, a replay buffer and Go live.";

export const OG_IMAGE_ALT =
  'The Cortex icon and name above "One key. One file." and "A screen and camera recorder for the Mac, with the camera on its own track.", beside a screenshot of the Cortex window with its sources sidebar, preview and red Record button.';
