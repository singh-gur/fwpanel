# fwpanel

A Linux desktop control panel for Framework laptops, using the official
[Framework System](https://github.com/FrameworkComputer/framework-system)
Rust library (`framework_lib`) through a privileged host service.

The approved initial scope is battery/AC status, charge-limit control, USB-C
power status, and supported input-deck/touchpad status. The first target is
Framework Laptop 13 AMD Ryzen AI 300 on Fedora 44 x86_64.

Built with:

- [Tauri 2](https://tauri.app) — desktop shell, Rust backend
- [SvelteKit 2](https://kit.svelte.dev) + Svelte 5 + TypeScript — UI
- pnpm — package manager

> Status: scaffold. The hello-world template; the architecture below is approved
> but not implemented. See the [development plan](plans/initial-development.md).

## Approved architecture (not yet implemented)

```
Svelte UI → Tauri commands → system D-Bus → fwpanel-service → framework_lib → hardware
```

The GUI stays unprivileged. A separately installed host service uses
`framework_lib` (initially pinned to 0.6.5) through the kernel EC driver.
Hardware logic remains upstream; fwpanel does not duplicate EC/ioctl code.
This replaces the original CLI-wrapper design: no CLI subprocesses, output
parsing, or CLI fallback.

The service checks polkit authorization for each operation. Active local users
can read status without password prompts; charge-limit changes require explicit
Apply and administrator authentication. Only fixed, named operations are exposed.
Rust handles service JSON and returns typed data to Svelte.

Distribution will be a native GUI RPM plus a Flatpak GUI, both using the
separately packaged host-service RPM. Flatpak does not install the privileged
service. AppImage and Flathub submission are deferred.

## Prerequisites

- Rust (rustup) and Node.js 20+ / pnpm
- Linux native deps for Tauri (webkit2gtk 4.1, librsvg, gcc-c++)

Fedora:

```bash
sudo dnf install webkit2gtk4.1-devel librsvg2-devel gcc gcc-c++
```

Other distros: see [Tauri's Linux prerequisites](https://tauri.app/start/prerequisites/).

The `framework_tool` CLI is not required. The planned hardware features will
require the separately installed `fwpanel-service`; that service does not exist
in the current scaffold yet.

## Development

```bash
pnpm install        # install JS dependencies
pnpm tauri dev      # run the full app with hot reload
```

Checks and builds:

```bash
pnpm check          # svelte-check + TypeScript diagnostics
pnpm build          # frontend only → build/
pnpm tauri build    # current scaffold bundles → src-tauri/target/release/bundle/
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

These commands and paths describe the current scaffold. The planned Cargo
workspace, service crates, and packaging recipes will be added during implementation.
See [AGENTS.md](AGENTS.md) for conventions and the
[development plan](plans/initial-development.md) for contracts and phase gates.

## Roadmap

- [ ] Host service, typed D-Bus contract, and polkit boundary
- [ ] Live battery/AC status and charge-limit reads through `framework_lib`
- [ ] Dashboard with polling, retry, and explicit stale/error states
- [ ] Authenticated charge-limit control with readback verification
- [ ] USB-C power and supported input-deck/touchpad status
- [ ] Native GUI RPM and separately installable host-service RPM
- [ ] Flatpak GUI using the host service

## License

MIT
