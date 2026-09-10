# AGENTS.md

Guidance for AI coding agents (and humans) working in this repository.

## What this repo is

`fwpanel` — a Linux desktop control panel for Framework laptops, using the
official `framework_lib` Rust library through a privileged host service.
Built with **Tauri 2** (Rust backend) + **SvelteKit** (TypeScript, Svelte 5)
using `@sveltejs/adapter-static` so the frontend compiles to static assets
embedded in the Tauri webview.

The frontend is Svelte 5 runes + TypeScript (strict): the dashboard
(`src/routes/+page.svelte`) polls `get_service_info`/`get_power` sequentially
(refresh on show, 5s after each finished refresh, paused while hidden,
manual Retry joins the in-flight refresh) and keeps typed DTO mirrors in
`src/lib/types.ts`. `crates/fwpanel-service` serves `GetServiceInfo` and
`GetPower` (battery/AC plus charge-limit reads through `framework_lib =0.6.5`,
kernel cros_ec driver only, supported only on Framework Laptop 13 AMD Ryzen AI
300); ports/deck/write return `unsupported_feature` until their phases. Follow
[plans/initial-development.md](plans/initial-development.md) for contracts and
owner-approved phase gates. First supported target: Framework Laptop 13
AMD Ryzen AI 300 on Fedora 44 x86_64.

## Approved architecture

```
Svelte component → invoke("cmd") → Tauri command → system D-Bus → fwpanel-service → framework_lib → hardware
```

This **replaces** the original CLI-wrapper design. Do not implement CLI
discovery, CLI subprocesses, text-output parsing, or CLI/port-I/O fallbacks.

The root `Cargo.toml` defines the workspace; `crates/fwpanel-protocol` holds
the shared serde DTOs/reply envelope/validation and `crates/fwpanel-service`
is the privileged host service (currently serving `GetServiceInfo` only;
hardware methods return `unsupported_feature` until their phases).

Rules for implementation:

- The GUI remains unprivileged and never accesses the EC, hardware sysfs, or
  `framework_lib` directly. Only the separately installed host service uses
  `framework_lib`, initially pinned to `=0.6.5` with default features disabled.
- Reuse upstream hardware logic through the kernel EC driver. Do not duplicate
  EC protocols/ioctl code, broaden device permissions, or silently switch drivers.
- Tauri handlers in `src-tauri/src/lib.rs` are thin typed D-Bus clients returning
  `Result<T, String>`. Rust decodes service JSON; Svelte receives typed data and
  never parses wire messages or hardware output.
- `crates/fwpanel-protocol` shares Rust DTOs/validation; `crates/fwpanel-service`
  owns hardware access and authorization. Both are workspace members of the
  root `Cargo.toml`; use the workspace (root) lockfile only — do not create
  per-crate lockfiles.
- The service authorizes the actual D-Bus sender with polkit on every operation.
  Active local users may read status without a prompt; inactive/remote sessions
  are denied. Only an explicit charge-limit Apply may request administrator
  authentication. Polling must never trigger authentication.
- Expose only the plan's named status and charge-limit methods, not arbitrary
  commands, paths, or EC requests. Validate writes in the service, preserve the
  existing minimum, verify readback, and never automatically retry a mutation.
- Keep blocking hardware work off UI/async executor threads, serialize it
  (single non-queuing gate in `crates/fwpanel-service/src/hardware.rs`), and
  follow the plan's timeout/recovery rules: a timed-out or panicking hardware
  worker terminates the service process (systemd restarts it); a timed-out
  future does not cancel a blocking hardware call. Do not present failures as
  valid zero/absent readings.
- No `sudo` invocations, privileged GUI, or user-selected helper executable.
  System installation and real hardware-write tests need separate owner approval.
- Deliver GUI RPM + Flatpak with a separately packaged host-service RPM.
  Flatpak gets narrow access to the service's D-Bus name, not direct hardware or
  unrestricted system-bus access. AppImage and Flathub submission are deferred.

## Commands

`just <task>` wraps the common ones (`just --list` to see all); raw equivalents:

| Task | Command |
|---|---|
| Install JS deps | `just install` / `pnpm install` |
| Type check frontend | `pnpm check` |
| Build frontend only | `pnpm build` (outputs to `build/`) |
| Dev (full app, hot reload) | `pnpm tauri dev` |
| Build release bundle | `pnpm tauri build` (outputs to `target/release/bundle/`) |
| Check Rust (workspace) | `cargo check --workspace` (from repo root) |
| Format Rust | `cargo fmt --all` (from repo root) |
| Lint Rust | `cargo clippy --workspace --all-targets -- -D warnings` |
| Rust tests | `cargo test --workspace` |
| Stage host service | `just stage-service <destdir>` (no root writes) |
| Check installed service | `just check-service` (read-only) |

There is no frontend test runner yet. Non-trivial conversion, validation, and
error logic gets unit tests; service authorization and hardware checks follow
the plan.

## Current layout

```
src/                  SvelteKit frontend (Svelte 5 runes, TypeScript)
  routes/+page.svelte Dashboard: refresh coordinator, service/battery/limit cards
  lib/types.ts        Strict TS mirrors of the protocol DTOs (keep in sync)
  app.html            Shell HTML (Tauri injects into this)
src-tauri/            Rust GUI backend
  src/lib.rs          Tauri commands and app wiring (main.rs only calls it)
  src/service.rs      Typed blocking D-Bus client for fwpanel-service
  tauri.conf.json     App config: window size, identifier io.github.singh-gur.fwpanel, build hooks
  capabilities/       Tauri permission capabilities — extend when invoking new Tauri APIs
crates/
  fwpanel-protocol/   Shared wire DTOs, reply envelope, validation, tests
  fwpanel-service/    Privileged host service: D-Bus + polkit + hardware gate
                      (battery/charge-limit reads live; ports/deck/write later)
packaging/            systemd/D-Bus/polkit assets, stage-service.sh, check-service.sh
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
- `target/` (workspace root) and `node_modules/` are git-ignored; never edit
  anything inside them. `build/` is a build artifact of `pnpm build` — do not
  commit changes to it. `stage/` (default `just stage-service` destination) is
  git-ignored staging output.
- After adding a Tauri plugin or API call in Rust, update
  `src-tauri/capabilities/` — missing capabilities are the usual cause of
  "command not allowed" runtime errors.
