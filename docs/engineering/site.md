# Landing page (`apps/site`)

The Cortex landing page is a fork of Rec's site, so it keeps Rec's layout, motion and code
style (SvelteKit 3, Svelte 5 runes, tabs in `.svelte` files). Only the brand, the copy and the
demo changed. It is a separate workspace app with no link to the web app's packages.

## Licence

Rec's repo is MIT OR Apache-2.0 and its `site/` folder has no licence of its own. The fork keeps
`LICENSE-MIT` and `LICENSE-APACHE` in `apps/site`, the root `NOTICE` credits it, and the footer
names Rec and its author. The first commit of `apps/site` is the upstream folder unchanged, from
rec-rs commit `93dd4948`.

## Where things live

| What                                                                                        | File                                                                    |
| ------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| Version, minimum macOS, architecture, URLs, title, description, release flags               | `src/lib/site.ts`                                                       |
| Page copy, feature cards, shortcuts, Quick trim keys, install steps                         | `src/routes/+page.svelte`                                               |
| Brand colours (hue 188 from `packages/ui/src/styles/studio.css`) and the app-window palette | `src/app.css`                                                           |
| Hero demo timeline, cursor path, captions, chapters                                         | `src/lib/demo/script.ts`                                                |
| The Cortex window mock in the demo                                                          | `src/lib/demo/CortexWindow.svelte`                                      |
| Demo sounds (Web Audio, no files)                                                           | `src/lib/demo/sound.ts`                                                 |
| Feature card visuals                                                                        | `src/lib/cards/*`                                                       |
| Shared mocks (cursor, camera picture, slide, track lanes, shape glyphs)                     | `src/lib/mock/*`                                                        |
| Visitor OS detection for the download buttons                                               | `src/lib/platform.ts`                                                   |
| `/download` redirect                                                                        | `src/routes/download/+server.ts` (target is `DOWNLOAD_URL`)             |
| Share image                                                                                 | `scripts/og/og.html` + `scripts/og/render.sh` → `src/lib/assets/og.jpg` |

## Rules for the copy

Every number on the page must be Cortex's own and traceable. Rec's measurements (CPU share, app
size, VMAF, frame counts, version, architectures) are never reused. Sources today:

- App size 22 MB: `du -sh /Applications/Cortex.app` on the 0.1.0 build.
- macOS 15 or later: `tauri.conf.json` `minimumSystemVersion`, `.cargo/config.toml`.
- Apple Silicon: `lipo -archs` on the app binary prints `arm64` only. Change `ARCH` when the
  release build is universal.
- 3840 × 2482 for 4K and the other resolution sizes: the table in `desktop.md`, "Resolution and
  bitrate", checked by `crates/rec-core/tests/fixtures/output-sizes.json`.
- 36,000 of 36,000 frames in a 10-minute 4K60 HEVC run: `rec-research/08-phase2-gate.md`.
- 2-second fragments, camera track layout, Quick trim modes, replay range (15–300 s), Go live
  services, hotkeys: `desktop.md` and `crates/rec-core/src/settings.rs`.

No CPU figure is shown: there is no recorded measurement of the current build in the repo. Add
one only with its source. Go live was tested against local servers only, so the page lists the
services without claiming they were verified on each platform.

## Placeholders

- `SITE_URL` (`https://cortex.example`), plus `static/robots.txt` and `static/sitemap.xml`.
- `DOWNLOAD_URL` (`https://cortex.example/releases/Cortex.dmg`).
- `NOTARIZED = false` adds the "Open Anyway" install step. Set it to true once releases are
  notarized. The step uses System Settings, because macOS 15 removed the Control-click → Open
  bypass.
- `WAITLIST_EMAIL` is empty, so the Windows and Linux notes have no link.
- The build uses `@sveltejs/adapter-vercel` from upstream. Nothing is deployed; swap the adapter
  when hosting is chosen.

## Checks

`pnpm --filter site check` and `build`. The repo's `oxlint` and `oxfmt` cover the `.ts`, `.css`,
`.json` and `.html` files; `.svelte` files are checked by svelte-check only.
