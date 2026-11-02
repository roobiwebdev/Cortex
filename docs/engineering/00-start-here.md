# Start here

These guides explain how Project Starter is built and why. The [README](../../README.md) tells you what to run. [CLAUDE.md](../../CLAUDE.md) is the short rulebook for AI agents. These pages give the reasons behind those rules.

## Read order

| #   | Page                                                               | What you learn                                                      |
| --- | ------------------------------------------------------------------ | ------------------------------------------------------------------- |
| 1   | [01-architecture.md](01-architecture.md)                           | The layers, the request flow and the composition root               |
| 2   | [02-procedures-and-routers.md](02-procedures-and-routers.md)       | Where your code goes, and the security checkpoint every call passes |
| 3   | [03-auth-and-permissions.md](03-auth-and-permissions.md)           | Better Auth, roles, permissions, organizations                      |
| 4   | [04-settings-and-integrations.md](04-settings-and-integrations.md) | The `app_settings` row, encrypted keys, R2, Resend, Polar           |
| 5   | [05-audit-rate-limit-logging.md](05-audit-rate-limit-logging.md)   | Audit log, rate limits and logging                                  |
| 6   | [06-adding-a-feature.md](06-adding-a-feature.md)                   | A full worked example, from schema to page                          |
| 7   | [07-types-and-grep.md](07-types-and-grep.md)                       | Type rules and how to find code fast                                |

Reference pages, read when you need them:

- [modules.md](modules.md): the module registry, and how to switch off or remove a module
- [database.md](database.md): Drizzle workflow, Neon, the driver, Better Auth schema
- [customizing.md](customizing.md): branding, themes, navigation, copy
- [deployment.md](deployment.md): Vercel, production migrations, webhooks
- [security.md](security.md): what the starter enforces and which Batman issues were fixed
- [desktop.md](desktop.md): the macOS recorder, its Rust crates, IPC, signing and CI

Outside this folder:

- [../SETUP.md](../SETUP.md): from zero to a running app
- [../ENVIRONMENT.md](../ENVIRONMENT.md): every environment variable

## If you only have five minutes

Read [01-architecture.md](01-architecture.md) for the mental model. Then read [06-adding-a-feature.md](06-adding-a-feature.md). Come back for the rest later.

## The five ideas that shape everything

1. **One security checkpoint.** Every data operation is a tRPC procedure built from `packages/api/src/trpc.ts`. Rate limiting, sign-in checks, role and permission checks, transactions and audit rows happen there, once.
2. **Env is read in one file.** `apps/web/src/server/services.ts` reads env and builds every client. Packages get what they need as arguments.
3. **Admins configure at runtime.** Feature toggles, site modes, branding and third-party keys live in one database row (`app_settings`), so most changes need no redeploy.
4. **One source of truth per concern.** Grep for `SOURCE OF TRUTH KEYWORDS` before you create anything.
5. **Modules can be switched off.** `packages/config/src/modules.ts` lists every prebuilt module, its routes and its navigation.
