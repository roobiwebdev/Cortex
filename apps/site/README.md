# Cortex landing page

A fork of Rec's site (the `site/` folder of github.com/michaelshimeles/rec-rs, MIT OR
Apache-2.0, © 2026 Michael Shimeles), rebranded for Cortex. The licence texts are in
`LICENSE-MIT` and `LICENSE-APACHE`; the root `NOTICE` credits it. SvelteKit, prerendered,
system fonts, no third-party requests.

```sh
pnpm --filter site dev       # http://localhost:3010
pnpm --filter site build
pnpm --filter site preview   # serves the build, including /download, on :3010
pnpm --filter site check     # svelte-check
pnpm --filter site og        # regenerate src/lib/assets/og.jpg (macOS + Google Chrome)
```

See `docs/engineering/site.md` for where each claim on the page comes from and what is
still a placeholder.
