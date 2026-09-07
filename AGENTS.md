# AGENTS.md

Guidance for AI coding agents (and humans) working in this repository.

## What this repo is

`fwpanel` — a Linux desktop control panel for Framework laptops, using the
official `framework_lib` Rust library through a privileged host service.
Built with **Tauri 2** (Rust backend) + **SvelteKit** (TypeScript, Svelte 5)
using `@sveltejs/adapter-static` so the frontend compiles to static assets
embedded in the Tauri webview.

The code is still the stock Tauri hello-world. The library-backed architecture
below is approved but **not implemented**. Follow
[plans/initial-development.md](plans/initial-development.md) for implementation
contracts, dependencies, and owner-approved phase gates. The first supported
target is Framework Laptop 13 AMD Ryzen AI 300 on Fedora 44 x86_64.

## Approved architecture (not yet implemented)

```
Svelte component → invoke("cmd") → Tauri command → system D-Bus → fwpanel-service → framework_lib → hardware
```

This **replaces** the original CLI-wrapper design. Do not implement CLI
discovery, CLI subprocesses, text-output parsing, or CLI/port-I/O fallbacks.

Rules for implementation:

- The GUI remains unprivileged and never accesses the EC, hardware sysfs, or
  `framework_lib` directly. Only the separately installed host service uses
  `framework_lib`, initially pinned to `=0.6.5` with default features disabled.
- Reuse upstream hardware logic through the kernel EC driver. Do not duplicate
  EC protocols/ioctl code, broaden device permissions, or silently switch drivers.
- Tauri handlers in `src-tauri/src/lib.rs` are thin typed D-Bus clients returning
  `Result<T, String>`. Rust decodes service JSON; Svelte receives typed data and
  never parses wire messages or hardware output.
- The planned `crates/fwpanel-protocol` shares Rust DTOs/validation;
  `crates/fwpanel-service` owns hardware access and authorization. These crates
  and the root Cargo workspace are Phase 1 deliverables, not existing paths.
- The service authorizes the actual D-Bus sender with polkit on every operation.
  Active local users may read status without a prompt; inactive/remote sessions
  are denied. Only an explicit charge-limit Apply may request administrator
  authentication. Polling must never trigger authentication.
- Expose only the plan's named status and charge-limit methods, not arbitrary
  commands, paths, or EC requests. Validate writes in the service, preserve the
  existing minimum, verify readback, and never automatically retry a mutation.
- Keep blocking hardware work off UI/async executor threads, serialize it, and
  follow the plan's timeout/recovery rules. A timed-out future does not cancel
  a blocking hardware call. Do not present failures as valid zero/absent readings.
- No `sudo` invocations, privileged GUI, or user-selected helper executable.
  System installation and real hardware-write tests need separate owner approval.
- Deliver GUI RPM + Flatpak with a separately packaged host-service RPM.
  Flatpak gets narrow access to the service's D-Bus name, not direct hardware or
  unrestricted system-bus access. AppImage and Flathub submission are deferred.

## Commands

These commands describe the **current scaffold**, not the future workspace.
Update commands and output paths when the corresponding phase actually changes them.
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

There is no frontend test runner yet. Rust tests run with `just test` or
`cargo test` inside `src-tauri/`. Non-trivial conversion, validation, and error
logic gets unit tests; service authorization and hardware checks follow the plan.

## Current layout

The planned service/protocol crates and packaging assets are listed in the plan;
do not assume they exist before their implementation phases.

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
- **TypeScript strict**; no `any`. App-owned response types live in Rust
  (shared serde DTOs once the protocol crate exists) and are re-declared in TS.
  Keep both sides in sync; do not expose upstream hardware structs directly.
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
