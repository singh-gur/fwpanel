# framework-dashboard

A Linux desktop GUI companion app for [framework-tool](https://github.com/Framework-Laptop/framework-tool),
built for Framework laptops.

The dashboard shells out to `framework-tool` and presents its output as a
native desktop UI — battery/charge state, charge limits, input ports, and other
Framework-specific controls as they gain CLI support.

Built with:

- [Tauri 2](https://tauri.app) — desktop shell, Rust backend
- [SvelteKit 2](https://kit.svelte.dev) + Svelte 5 + TypeScript — UI
- pnpm — package manager

> Status: scaffold. The hello-world template; no framework-tool integration yet.

## Architecture

The GUI is a view layer only. All hardware interaction goes through the
`framework-tool` CLI, invoked from small Rust command handlers:

```
Svelte UI → invoke("cmd") → Rust #[tauri::command] → framework-tool subprocess → serde JSON → UI
```

One source of truth for hardware logic, no duplicated EC/ioctl code, and the
CLI's permission handling is reused as-is.

## Prerequisites

- Rust (rustup) and Node.js 20+ / pnpm
- Linux native deps for Tauri (webkit2gtk 4.1, librsvg, gcc-c++)

Fedora:

```bash
sudo dnf install webkit2gtk4.1-devel librsvg2-devel gcc gcc-c++
```

Other distros: see [Tauri's Linux prerequisites](https://tauri.app/start/prerequisites/).

The `framework-tool` binary must be on `$PATH` (will become a runtime check).

## Development

```bash
pnpm install        # install JS dependencies
pnpm tauri dev      # run the full app with hot reload
```

Checks and builds:

```bash
pnpm check          # svelte-check + TypeScript diagnostics
pnpm build          # frontend only → build/
pnpm tauri build    # release bundle (AppImage/rpm) → src-tauri/target/release/bundle/
```

Rust-only checks (from `src-tauri/`):

```bash
cargo fmt
cargo clippy
cargo test
```

## Project layout

```
src/           SvelteKit frontend (UI, routes)
src-tauri/     Rust backend (commands, app config, capabilities)
static/        Static assets
```

See [AGENTS.md](AGENTS.md) for detailed conventions and workflow rules.

## Roadmap

- [ ] Runtime detection of `framework-tool` (version + path check)
- [ ] Battery/charge state card (read-only, polled)
- [ ] Charge limit control (requires polkit escalation)
- [ ] Input port / module info
- [ ] Packaging: rpm + AppImage

## License

MIT
