# Project Starter: rules for AI coding agents

Every rule here is mandatory. The "why" behind each rule is in `docs/engineering/`.
`AGENTS.md` points here so every agent reads the same rules.

## 0. How to work

1. **Plan before code.** For any feature, use the `architect-feature` skill. Restate the goal, break it into building blocks, find what already exists, place it at the root, run the security pass, and propose a build order. Get approval before writing implementation code.
2. **Explore cheaply.** Grep for `SOURCE OF TRUTH KEYWORDS` and similar names before reading whole files. Read only what you need.
3. **Reuse before you create.** Never create a type, function, component, constant or pattern without first grepping for an existing one. Duplicates are bugs.
4. **Root cause, not symptoms.** Fix problems where they originate so every caller benefits.
5. **Finish the whole feature.** Never leave a piece half-built. If something is out of scope or blocked, say so explicitly in your handover.
6. **Prove it.** Before handing over, run `pnpm check-types` and `pnpm lint`, plus the tests for what you touched (`pnpm test`). Report real results. Never claim a pass you did not see.
7. **Git: task branches only.** For edits, use the `new-feature` skill: one worktree and branch per task, created from `origin/main` inside the gitignored `.worktrees/` folder. Agents may create worktrees and branches, commit, and push their **own task branch**. Never commit or push to `main`, never merge, never `--force` (only `--force-with-lease` on your own branch), and open a PR only when the user asks. Read-only work (reviews, explanations, investigations) needs no worktree.

## 1. Architecture: where code goes

```
apps/web            Next.js 16 app (pages, layouts, route handlers, server actions)
apps/native         Expo app (talks to the same tRPC API)
apps/docs           Fumadocs documentation site
apps/desktop        Tauri 2 macOS recorder: React surfaces in src/, Rust shell in src-tauri/
apps/site           SvelteKit landing page for the recorder, forked from Rec's site (keeps its own style; see docs/engineering/site.md)
crates/rec-core     Recorder settings schema, engine API and state (platform-neutral, no Apple code)
crates/rec-engine-macos   Rec's capture/encode engine (ScreenCaptureKit, AVFoundation, VideoToolbox)
crates/rec-macos-ui Native window behaviour: panels, levels, capture exclusion
crates/rec-cli      The rec-rs command-line recorder
packages/db         Drizzle schema, relations, migrations  (database shape only)
packages/core       Server building blocks: settings, secrets, email, storage, payments, notify, logger
packages/auth       Better Auth factory + global roles/permissions
packages/api        tRPC: procedure builder (security checkpoint), routers, services
packages/config     Module registry, branding, themes, shared option lists (no runtime deps)
packages/ui         shadcn/ui primitives + theme tokens (globals.css)
```

Request flow: **UI → server action or tRPC hook → router (business logic) → service / ctx.db (data) → Postgres.**

- **Env is read in exactly one place:** `apps/web/src/server/services.ts` (via `server/env.ts`). Packages receive config and clients as arguments. Never read `process.env` in a package. (Existing exceptions, do not add more: `NODE_ENV` checks in `packages/core/src/logger.ts` and the tRPC error formatter, and the database CLI tooling in `packages/db/drizzle.config.ts` and `src/reset.ts`.)
- **Every data operation goes through a tRPC procedure** from `packages/api/src/trpc.ts`:
  - `publicProcedure()` for anonymous reads and public forms (rate limited by IP, or by user ID when signed in; mutations are audited only if you pass `audit`)
  - `protectedProcedure({ role?, permission?, rateLimit?, audit? })` for signed-in users
  - `adminProcedure()` for admin-only endpoints
    This builder is the security checkpoint. It runs rate limiting, authentication, role and permission checks. For mutations it also opens a transaction and writes the audit row. Never re-implement these checks in a handler.
- **Routers** (`packages/api/src/routers/*.ts`) hold business logic: validation, orchestration, decisions. Name procedures `<router>.<verb>`. Mutations are audited automatically as `entity=<router>, action=<verb>`.
- **Services** (`packages/api/src/services/*.ts`) hold reusable database queries only. No auth, no business rules. Add `import "server-only";` at the top. Import them into routers with `import * as fooService from "../services/foo"`. Trivial one-off queries can live inline in the router using `ctx.db`.
- **Server actions** (`apps/web/src/lib/actions/*.ts`) are thin adapters: `"use server"`, then call `(await api()).<router>.<proc>(input)`. No logic, no direct database access.
- **Server Components** call `await api()` from `@/server/trpc` or the session helpers in `@/server/session`.
- **Client components** use existing server actions, or `useTRPC()` from `@/lib/trpc/client` with TanStack Query.
- **Route handlers** (uploads, webhooks, auth) use `getServices()`. Keep them thin.
- **Next 16 uses `src/proxy.ts`. Never create `middleware.ts`.**

### Desktop and Rust

Desktop request flow: **React surface → `lib/engine.ts` → generated `ipc/bindings.ts` → Tauri command in `src-tauri/src/ipc.rs` (the single IPC checkpoint) → `rec_core` API → `MacEngine`.** Details are in `docs/engineering/desktop.md`.

- **`rec-core` is the settings SOT.** Change a setting, state field or engine event there, then regenerate the bindings.
- **IPC types are generated.** Never hand-edit `apps/desktop/src/ipc/bindings.ts`. Run `pnpm --filter desktop ipc:bindings` and commit the result with the Rust change.
- **Capabilities are an allow-list.** A new command goes in `COMMANDS` (`src-tauri/build.rs`) and gets an `allow-<command>` grant in `src-tauri/capabilities/`. No shell, fs or http plugins for the webview. No remote content.
- **No secrets over IPC.** Stream keys stay in the Keychain. The webview only sees whether a key is stored.
- **`unsafe` only in `rec-engine-macos` and `rec-macos-ui`**, each block with a `// SAFETY:` comment.
- **New crates pass `cargo fmt --check` and `cargo clippy -- -D warnings`.** The imported Rec crates keep upstream style.
- The TypeScript rules (section 3) and UI rules (section 6) apply to `apps/desktop/src`. Recorder tokens live in `packages/ui/src/styles/studio.css`.

## 2. Single sources of truth (grep these first)

| Concern                                                         | Lives in                                                                     |
| --------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| Database tables and types                                       | `packages/db/src/schema/*` (`typeof table.$inferSelect`)                     |
| Runtime settings, feature toggles, site modes, integration keys | `app_settings` via `ctx.services.settings` (`packages/core/src/settings.ts`) |
| Which modules exist, nav items                                  | `packages/config/src/modules.ts`                                             |
| Branding defaults                                               | `packages/config/src/branding.ts`                                            |
| Theme presets                                                   | `packages/config/src/themes.ts` + `packages/ui/src/styles/globals.css`       |
| Roles and permissions                                           | `packages/auth/src/permissions.ts`                                           |
| Rate-limit presets                                              | `packages/api/src/lib/rate-limit.ts`                                         |
| Notification tags                                               | `packages/core/src/types.ts`                                                 |
| Feedback categories and statuses                                | `packages/config/src/content.ts`                                             |
| Env variables                                                   | `apps/web/.env.schema` (Varlock). Add new variables here first.              |
| API input/output types for clients                              | `RouterInputs` / `RouterOutputs` from `@starter/api`                         |
| Recorder settings schema, engine API and state                  | `crates/rec-core/src/{settings,api,state}.rs`                                |
| Desktop IPC types (generated, never hand-edit)                  | `apps/desktop/src/ipc/bindings.ts` from `src-tauri/src/ipc.rs`               |
| Desktop IPC allow-list                                          | `apps/desktop/src-tauri/capabilities/` + `COMMANDS` in `src-tauri/build.rs`  |
| Recorder design tokens                                          | `packages/ui/src/styles/studio.css`                                          |
| Recorder window behaviour (panels, levels, capture exclusion)   | `crates/rec-macos-ui`                                                        |

## 3. TypeScript (production grade)

- No `any`. No `as` assertions, except `as const` and the `as never` needed for Next typed routes on dynamic hrefs. No `!` non-null assertions. No `@ts-ignore` or `@ts-expect-error`. Narrow `unknown` with type guards, never by casting.
- **Type lookup order:** Drizzle inferred types, then Better Auth `$Infer` (`Session`, `AuthUser` from `@starter/auth`), then router-derived types (`RouterOutputs`), then existing SOT types (grep). Only then write a new type, in the owning package's `types.ts`, with a SOT header.
- Strict config is on (`strict`, `noUncheckedIndexedAccess`). Fix the real type instead of fighting it.

## 4. Validation and security

- **Zod on every input boundary:** every procedure `.input()`, every route handler body, every form.
- Forms use **TanStack Form** (`@tanstack/react-form`) with Zod validators, matching the existing forms.
- Secrets never reach the client. Integration keys are read through `settings.getIntegrationKeys()` (server-only). The admin UI only receives masked values.
- New third-party secrets: add an encrypted column (`SECRET_KEYS` in `packages/core/src/types.ts`), an env fallback in `.env.schema`, and wire it in `services.ts`.
- Escape user text in emails with `escapeHtml`. Sanitize rendered rich text (blog) on the server.
- Never log secrets, tokens or full request bodies. Use `createLogger(scope)` from `@starter/core/logger`.

## 5. Source-of-truth comments

Put this header above every exported, reusable block (functions, components, schemas, services):

```ts
/**
 * SOURCE OF TRUTH KEYWORDS: Symbol1, Symbol2, related words (around 6 to 10)
 * WHAT:  what this is
 * WHY:   why it exists and why it is built this way
 * WHERE: where it is used
 */
```

Inline comments explain outcomes and intent, not syntax. Keep them short.

## 6. UI rules

- Check `packages/ui/src/components` and `apps/web/src/components` before creating a component. Extend or compose instead of duplicating.
- Reused across routes: `apps/web/src/components/<name>` (or `packages/ui` for design-system primitives). Used by one route only: `<route>/_components/`.
- Global components take props and slots. No hardcoded data or copy inside them.
- **Theme tokens only:** `bg-background`, `text-foreground`, `text-muted-foreground`, `bg-card`, `bg-primary`, `border`, and so on. No hex values, no `text-white`, no `bg-[#...]`, no forced `dark` classes.
- When importing shadcn blocks, fix their copy and branding to match the app.
- Complex new components: ask the user for a shadcn block link to use as reference.
- Icons: `lucide-react`.

## 7. Database changes

1. Edit `packages/db/src/schema/*.ts` and update `relations.ts`.
2. Run `pnpm db:generate` to create a SQL migration. Review the SQL, and commit it with the schema change.
3. Run `pnpm db:migrate` to apply it. Use `db:push` only for throwaway prototyping, never for shared databases.
4. Better Auth tables: after changing auth plugins or `additionalFields`, run `pnpm auth:check`. It fails and lists any table or column Better Auth expects that is missing from `packages/db/src/schema`. Add them, then generate a migration.
5. Seeds belong in `apps/web/scripts/seed.ts` and must be idempotent. This is the only allowed script for manual data changes. Ask the user before adding any other script.

## 8. Adding or removing a module

See `docs/engineering/modules.md`. In short, a module is:

- schema file(s)
- a router mounted in `packages/api/src/root.ts`
- pages under `apps/web/src/app/...`
- an entry in `packages/config/src/modules.ts`

Set `enabled: false` to switch a module off. `proxy.ts` then returns 404 for its routes, and its nav entries disappear.

## 9. Commands

```
pnpm bootstrap          first-run setup (env, database, migrate, seed)
pnpm dev                web app on http://localhost:3001
pnpm check              lint + format check + type check + tests
pnpm test:integration   API tests against a real database (needs TEST_DATABASE_URL)
pnpm auth:check         verify the Drizzle schema has every table/column Better Auth expects
pnpm db:generate | db:migrate | db:seed | db:reset | db:studio
pnpm build              production build (web + docs)
pnpm rea <command>      REA reverse-engineering CLI (also the `rea` MCP server in .mcp.json)

pnpm --filter desktop dev                   recorder UI in the browser on http://localhost:3002 (mock engine)
pnpm --filter desktop tauri build --debug   debug .app (sign with APPLE_SIGNING_IDENTITY="Cortex Dev Signing")
pnpm --filter desktop ipc:bindings          regenerate apps/desktop/src/ipc/bindings.ts
cargo test --workspace                      Rust tests (needs rustup on PATH)
cargo run --release -p rec-engine-macos --example engine_smoke   test-pattern recording + ffprobe check

pnpm --filter site dev                      landing page on http://localhost:3010
pnpm --filter site build                    prerendered build (+ preview on :3010 with `pnpm --filter site preview`)
pnpm --filter site check                    svelte-check
pnpm --filter site og                       regenerate the share image (needs Google Chrome)
```

To study how a feature works in an existing app or binary before building our own version, use the `reverse-engineer-anything` skill with the `rea` MCP server. Rebuild the behavior in this codebase following the rules above; never copy third-party code.

## 10. Workflow skills: isolate, build, prove, ship

Pick the steps a task needs. A plan, review or local experiment does not imply a branch, commit or PR.

| Step     | Skill                           | Starter rule                                                                                                                                                                                                                                                                            |
| -------- | ------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Plan     | `architect-feature`             | Section 0, rule 1.                                                                                                                                                                                                                                                                      |
| Isolate  | `new-feature`                   | Section 0, rule 7. Worktrees go in `.worktrees/<task-name>`, branches are `agent/<task-name>`. Run `pnpm install` inside the worktree.                                                                                                                                                  |
| Build    | `architect-feature` + this file | The starter's routers/services rules (section 1) replace the generic "actions vs services" advice found elsewhere.                                                                                                                                                                      |
| Prove    | `evidence-driven-testing`       | Run `pnpm check` plus the checks for what you touched. For changed UI or runtime behavior, record the failure before the fix and the success after. Evidence goes in the gitignored `.artifacts/<task-name>/`. Playwright specs live in `apps/web/e2e/` (`pnpm --filter web test:e2e`). |
| Ship     | `before-and-after`              | Screenshots stay local by default. Upload (the default host 0x0.st is public) only when the user asks for PR images, and never for pages showing private data.                                                                                                                          |
| Write    | `unslop`                        | Run it over prose you write or edit for a person: commit messages, PR titles and bodies, docs, code comments. Leave text you didn't touch alone.                                                                                                                                        |
| Research | `reverse-engineer-anything`     | See section 9.                                                                                                                                                                                                                                                                          |

**Multi-agent rules**

- One worktree and one branch per task and per agent. Never touch another agent's worktree, branch or uncommitted work.
- Before starting, skim open PRs (`gh pr list`, `gh pr diff <n> --name-only`). Independent edits to the same file are fine in your own worktree. Ask when changes conflict in behavior, depend on unfinished work, or ownership is unclear.
- Resolve `pnpm-lock.yaml` conflicts by regenerating it (`pnpm install`), never by hand-merging.
- Worktrees do not isolate ports or databases. Confirm a dev-server port answers your process (`lsof -i :3001`) before trusting it, and never run schema experiments against a shared database.
- If a conflict can't be resolved confidently, stop and report instead of guessing.

**Shipping a task (only when asked to ship):** keep the change to the task, run the checks, commit with a clear message, rebase onto `origin/main` and re-run the checks, push your branch, then open the PR with what changed, how it was tested (each claim backed by evidence), before/after proof, and risks. Do not merge unless told to.

Keep this file and `docs/engineering/` updated when you change structure, commands or conventions.
