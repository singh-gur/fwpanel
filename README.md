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

> Status: Phase 1 of the [development plan](plans/initial-development.md) is
> implemented — Cargo workspace, shared protocol crate, privileged host service
> skeleton with polkit authorization, and packaging assets (pending owner
> acceptance). The UI is still the template; hardware features and packaging
> land in later phases.

## Approved architecture (service boundary implemented)

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

- Rust (rustup) and Node.js 24 / pnpm 12+
- Linux native deps for Tauri (webkit2gtk 4.1, librsvg, gcc-c++)

Fedora:

```bash
sudo dnf install webkit2gtk4.1-devel librsvg2-devel gcc gcc-c++
```

Other distros: see [Tauri's Linux prerequisites](https://tauri.app/start/prerequisites/).

The `framework_tool` CLI is not required and never invoked. Hardware features
need the separately installed `fwpanel-service` (see below); until it is
installed, the GUI reports the service as unavailable.

## Development

```bash
pnpm install        # install JS dependencies
pnpm tauri dev      # run the full app with hot reload
```

Checks and builds (workspace-aware):

```bash
pnpm check              # svelte-check + TypeScript diagnostics
pnpm build              # frontend only → build/
just check-all          # frontend + cargo fmt/clippy/test for the whole workspace
pnpm tauri build        # release bundle → target/release/bundle/
```

Rust commands (`cargo check`/`fmt`/`clippy`/`test`) run from the repo root and
cover the whole workspace: `src-tauri` plus `crates/fwpanel-protocol` and
`crates/fwpanel-service`.

## Host service staging and installation

`just stage-service <destdir>` builds the service and stages the executable
plus its system assets under a caller-owned directory. It performs **no**
privileged operations. Installing is a separate, explicit owner action; these
are the five files and their system paths:

| Staged file | System path |
|---|---|
| `usr/libexec/fwpanel-service` | `/usr/libexec/fwpanel-service` (0755) |
| `usr/lib/systemd/system/fwpanel-service.service` | same under `/usr/lib/systemd/system/` (0644) |
| `usr/share/dbus-1/system-services/io.github.singh_gur.Fwpanel1.service` | same under `/usr/share/dbus-1/system-services/` (0644) |
| `usr/share/dbus-1/system.d/io.github.singh_gur.Fwpanel1.conf` | same under `/usr/share/dbus-1/system.d/` (0644) |
| `usr/share/polkit-1/actions/io.github.singh_gur.fwpanel.policy` | same under `/usr/share/polkit-1/actions/` (0644) |

Installation/removal is performed by the machine's owner in an administrator
session: copy the five files to the listed paths (root-owned, non-user-
writable), then `sudo systemctl daemon-reload` (and `sudo systemctl reload
dbus` so the new D-Bus policy is picked up). The service is D-Bus activated —
do not enable it at boot. `just check-service` runs read-only introspection
and a `GetServiceInfo` call against an installed service.

## Project layout

```
src/                    SvelteKit frontend (UI, routes)
src-tauri/              Rust GUI backend (Tauri commands, typed D-Bus client)
crates/fwpanel-protocol/  Shared wire DTOs, reply envelope, validation
packaging/              systemd/D-Bus/polkit assets and stage/check scripts
static/                 Static assets copied verbatim
```

Cargo resolves the whole tree as one workspace from the root `Cargo.toml`.
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
