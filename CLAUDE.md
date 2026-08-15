# personal-swiss-knife

Personal desktop "swiss knife" app. Tauri v2 (Rust backend) + React 19 + Vite + TypeScript.

## Stack / conventions (get these right the first time)

- **Package manager is `pnpm`** — never `npm`/`yarn`. Lockfile is `pnpm-lock.yaml`.
- **Lint/format is oxlint + oxfmt**, NOT eslint/prettier. Configs: `.oxlintrc.json`, `.oxfmtrc.json`.
  - `pnpm check` = verify all (oxfmt + oxlint + `cargo fmt --check` + `cargo clippy -D warnings`).
  - `pnpm fix` = auto-fix all (JS + Rust). Run before claiming done.
- **Rust clippy runs with `-D warnings`** — a warning fails the build. Fix, don't ignore.
- **Frontend import alias**: `#*` → `./src/*` (package.json `imports`). Use `#components/...`, not deep relative paths.
- **Routes are file-based TanStack Router** in `src/routes/*.tsx`. Data via TanStack Query. UI: `@base-ui/react` + shadcn + Tailwind v4.
- **Run/build**: `pnpm tauri dev` for the app; `pnpm build` = `tsc && vite build` (frontend only).

## Layout

- `src/routes/` — one file per page (deploy, memory, messenger, pull-requests, ssh, utils).
- `src-tauri/src/<feature>/` — Rust per feature: `devkon`, `github`, `memtrack`, `messenger`, `ssh`, `utils`. Commands registered in `lib.rs`.
- Tauri commands: define in `<feature>/commands.rs`, register in `lib.rs`, add capability in `src-tauri/capabilities/`.

## Repeated mistakes — DO NOT repeat (append when you catch a new one)

- Reaching for `npm`/`eslint`/`prettier`. This repo is `pnpm` + oxlint + oxfmt.
- Skipping the Rust side of checks — clippy `-D warnings` will fail CI even when JS is clean.
- <!-- add real recurring mistakes here as they happen -->

## Keep this file current

If you learn something that matters for **every** future session (a convention, a gotcha, a
recurring mistake, a build quirk), add it here — short, in the right section above. This file is
loaded every session; that is the only reason to put something here.

## Lazy-loaded docs (read on demand, not every session)

Detailed, feature-specific notes live in [docs/memory/](docs/memory/). They are NOT auto-loaded.
Read the relevant file only when working on that area. Index: [docs/memory/README.md](docs/memory/README.md).
When a fact is deep/feature-specific (not needed every session), write it there and link it, not here.
