# rec-rs site

The download page for Rec. SvelteKit with `adapter-vercel`; the page is prerendered. Run the
commands from this folder:

```sh
npm install
npm run dev       # http://localhost:5173
npm run build
npm run preview   # serves the production build, including /download
```

`npm install` and the build write `node_modules/`, `.svelte-kit/` and `.vercel/` into this
folder. The repo is synced between machines (Mutagen), so run them in a clone or worktree
outside the synced checkout.

- Deploy it as a Vercel project with the Root Directory set to `site`.
- `/download` (`src/routes/download/+server.ts`) answers with a 302 to
  `https://github.com/michaelshimeles/rec-rs/releases/latest/download/rec-rs-universal.dmg`.
  Every GitHub release must include an asset with exactly that name.
- The version, URLs, title and description live in `src/lib/site.ts`. Update `VERSION` with
  each release. If the site gets another domain, update `SITE_URL` (the canonical, Open Graph
  and JSON-LD tags use it) and the domain in `static/robots.txt` and `static/sitemap.xml`.
- `static/og.jpg` is the 1200x630 share image. Edit `scripts/og/og.html` and run
  `scripts/og/render.sh` to regenerate it; it needs macOS (for `sips`), Google Chrome and
  `npm install`.
- The screenshots in `src/lib/assets/` are the Rec window at 1200x800 pt, captured from the app
  in light and dark appearance and encoded as WebP at 2160 and 1200 px wide.
- Fonts (Geist, Geist Mono) are bundled from `@fontsource-variable`, so the page makes no
  third-party requests.
