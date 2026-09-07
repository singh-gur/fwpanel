# AGENTS.md

Guidance for AI coding agents (and humans) working in this repository.

## What this repo is

`fwpanel` — a Linux desktop GUI control panel for the `framework-tool`
CLI on Framework laptops. Built with **Tauri 2** (Rust backend) + **SvelteKit**
(TypeScript, Svelte 5) using `@sveltejs/adapter-static` so the frontend compiles
to static assets embedded in the Tauri webview.

This is an early scaffold: the stock Tauri hello-world. Features are added in
small increments.

## Core architecture decision

The GUI does **not** talk to the embedded controller or sysfs directly. All
hardware interaction goes through the `framework-tool` CLI, invoked from Rust:

```
Svelte component → invoke("cmd") → Rust #[tauri::command] → framework-tool (subprocess) → serde JSON → back to UI
```

Rules:

- Rust commands in `src-tauri/src/lib.rs` shell out to `framework-tool`
  (`std::process::Command`), parse stdout as JSON, and return typed structs
  (`serde::Deserialize`/`Serialize`). Never parse CLI output in JS.
- Every command returns `Result<T, String>` so errors surface in the UI.
- If `framework-tool` gains new subcommands, mirror them as new Tauri commands —
  one thin wrapper each, no abstraction layers over `Command`.
- Operations that need root will require polkit integration later; until then,
  read-only commands only. Do not add `sudo` invocations.

## Commands

`just <task>` wraps the common ones (`just --list` to see all); raw equivalents:

| Task | Command |
|---|---|
| Install JS deps | `just install` / `pnpm install` |
| Type check frontend | `pnpm check` |
| Build frontend only | `pnpm build` (outputs to `build/`) |
| Dev (full app, hot reload) | `pnpm tauri dev` |
| Build release bundle | `pnpm tauri build` (outputs to `src-tauri/target/release/bundle/`) |
| Check Rust only | `cargo check` (run inside `src-tauri/`) |
| Format Rust | `cargo fmt` (inside `src-tauri/`) |
| Lint Rust | `cargo clippy` (inside `src-tauri/`) |

There is no test runner yet. Non-trivial Rust parsing logic gets a unit test
module in the same file; trivial wrappers get none.

## Layout

```
src/                  SvelteKit frontend (Svelte 5 runes, TypeScript)
  routes/             Pages — single-window dashboard lives here
  app.html            Shell HTML (Tauri injects into this)
src-tauri/            Rust backend
  src/lib.rs          All #[tauri::command]s and app wiring (main.rs only calls it)
  tauri.conf.json     App config: window size, identifier io.github.singh-gur.fwpanel, build hooks
  capabilities/       Tauri permission capabilities — extend when invoking new Tauri APIs
static/               Static assets copied verbatim
```

## Conventions

- **Svelte 5 runes**: use `$state`, `$derived`, `$effect`. No legacy stores, no
  `on:click` (use `onclick`). Components are `.svelte.ts`-free; UI logic lives
  in `.svelte` files, shared logic in `src/lib/*`.
- **TypeScript strict**; no `any`. Types describing CLI output live in Rust
  (serde structs) and are re-declared in TS — keep both sides in sync when
  changing a command's return shape.
- **Calling Rust from JS**: `import { invoke } from "@tauri-apps/api/core"`.
- No new npm/cargo dependencies without justification — the stack is
  intentionally minimal.
- Keep diffs small. Fix the reported issue; don't refactor adjacent code.
- Commit style: conventional commits (`feat:`, `fix:`, `chore:`, `docs:`).

## Environment notes

- pnpm is the package manager (v12+). Never use npm/yarn commands.
- Linux system deps for Tauri (webkit2gtk, librsvg, gcc-c++). On Fedora:
  `sudo dnf install webkit2gtk4.1-devel librsvg2-devel gcc gcc-c++`.
- Node via nvm; Rust via rustup. Cargo binaries land in `~/.cargo/bin`.
- `src-tauri/target/` and `node_modules/` are git-ignored; never edit anything
  inside them. `build/` is a build artifact of `pnpm build` — do not commit
  changes to it.
- After adding a Tauri plugin or API call in Rust, update
  `src-tauri/capabilities/` — missing capabilities are the usual cause of
  "command not allowed" runtime errors.
