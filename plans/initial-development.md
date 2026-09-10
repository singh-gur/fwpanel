# Implementation Plan: Initial fwpanel Development

## Overview

Build a useful Framework laptop control panel for the owner's Framework Laptop 13 (AMD Ryzen AI 300 Series), Ryzen AI 9 HX 370, running Fedora 44 x86_64. Deliver battery/AC status, a charge-limit control, USB-C power status, and supported input-deck/touchpad status.

Ship a native GUI RPM and a Flatpak GUI, both using a separately packaged, trusted host service. This is an approved implementation plan, not authorization to install system components, change hardware settings, or publish packages without the corresponding owner approval.

## Planning Profile

- Executor: **Workhorse**.
- Detail: **Standard**; implementation contracts are explicit to avoid rediscovering architecture.
- Feedback: **Standard**; confirm material deviations and review each phase.
- Mode: **Phased**; each phase produces an independently verifiable checkpoint.
- Plan destination: `plans/initial-development.md`.
- Approval: owner approved the draft direction in the planning conversation.
- Implementation status: Phase 1 implemented on `feat/initial-01-service`,
  pending owner acceptance. Later phases not started.

## Global Context

### Observed repository facts

- Planning baseline: clean `main`, tracking `origin/main`, commit `59b3e40ffd0d04e34aab1c78916b01c2087f9fc5`.
- Frontend: SvelteKit 2, Svelte 5 runes, TypeScript strict, static adapter, SSR disabled. The single page is the stock greeting template in `src/routes/+page.svelte`.
- Backend: Tauri 2; `src-tauri/src/lib.rs` registers only `greet`. Existing dependencies include serde and serde_json.
- `src-tauri/tauri.conf.json` currently disables CSP and selects all bundle targets. The default capability includes the template opener permission.
- `justfile` already wraps checking, building, formatting, linting, and Rust tests. `check-all` does not currently include formatting verification, frontend build, packaging, or hardware validation.
- Installed tools observed: Node 24.20.0, pnpm 12.3.4, Rust 1.98.1, and Flatpak 1.18.2. `flatpak-builder` and `rpmbuild` were not found on PATH.
- `pnpm check` passed with zero errors/warnings; `cargo fmt --manifest-path src-tauri/Cargo.toml --check` passed. Full Rust compilation, desktop launch, packaging, and privileged hardware behavior were not verified during planning.

### Observed integration facts

- The installed official CLI is `/usr/bin/framework_tool`, package `framework-system-0.6.5-1.fc44.x86_64`; the scaffold's hyphenated executable/upstream-link assumptions are inaccurate for this installation.
- Local unprivileged battery, charge-limit, input-deck, and port reads failed. Some failed CLI operations nevertheless returned exit code zero.
- Published `framework_lib` 0.6.5 exists and exposes typed APIs for the selected features. Its minimum Rust version is 1.81; optional default features include hidapi/rusb.
- `/dev/cros_ec` exists on the target laptop. Existence is not proof that every selected EC operation works.
- Upstream hardware paths contain `unwrap`, indexing, and arithmetic that can panic on unexpected data. The service boundary must contain failures; frontend deadlines alone do not cancel blocking library calls.

## Architecture Decisions

### Approved replacement for the scaffold's CLI-only design

```text
Svelte UI
  -> typed Tauri commands
  -> system D-Bus
  -> fwpanel-service + polkit
  -> framework_lib
  -> kernel EC driver / upstream hardware access
```

- Replace, rather than supplement, CLI integration. No CLI detection, subprocess text parsing, or CLI fallback is required.
- Pin `framework_lib` to `=0.6.5` initially, with default features disabled. Only the host service depends on it.
- Reuse upstream hardware implementation; do not write EC protocols, ioctl wrappers, raw register access, or parallel sysfs battery readers.
- Explicitly select `CrosEcDriverType::CrosEc` using `CrosEc::with`. If unavailable, report an unavailable driver. Do not fall back to port I/O, change device permissions, disable security features, or request broad capabilities to make a fallback work.
- The initial supported platform is `smbios::Platform::Framework13AmdAi300`, obtained through upstream platform detection. Other/unknown platforms return unsupported status and never enable writes.
- Keep `src-tauri` as the GUI crate. Add `crates/fwpanel-service` for the host executable and `crates/fwpanel-protocol` for shared serde types and validation, under a root Cargo workspace. The protocol crate contains no hardware or authorization implementation.
- Use established Rust D-Bus support (`zbus` 5 API family) and a Tokio service runtime. These dependencies are justified by the approved service/polkit boundary, not a general plugin architecture. Resolve exact compatible releases through Cargo and commit the lockfile.
- Retain the existing Svelte stack; add no UI/component library or frontend test framework initially.

### D-Bus and data contract

Use these names consistently in service code, the Rust client, system policy, tests, and Flatpak permissions:

- Bus name: `io.github.singh_gur.Fwpanel1`.
- Object path: `/io/github/singh_gur/Fwpanel1`.
- Interface: `io.github.singh_gur.Fwpanel1`.
- Methods: `GetServiceInfo()`, `GetPower()`, `GetPorts()`, `GetInputDeck()`, and `SetChargeLimit(maximum: u32)`.
- No general command, argument-vector, EC-command, filesystem-path, or firmware-operation method.

Each method returns a JSON string encoded from shared Rust types. This avoids D-Bus-specific optional-value representations while keeping both Rust endpoints typed. The GUI backend deserializes it; Svelte never parses service strings or upstream output.

Define in `crates/fwpanel-protocol/src/lib.rs`:

- Protocol major constant `1`; every reply is tagged `ok` or `error` and includes `protocol_version`.
- Success replies contain the method's typed `data`; error replies contain a stable error `code` and a safe human-readable `message`.
- Error codes: `access_denied`, `unsupported_platform`, `driver_unavailable`, `unsupported_feature`, `hardware_unavailable`, `invalid_data`, `invalid_argument`, `busy`, and `outcome_unknown`. Connection/protocol errors are additionally mapped by the Tauri client.
- `ServiceInfo`: service version, protocol version, pinned library version, and implemented feature names. It does not probe hardware or expose serial numbers. An implemented feature is not a claim of firmware support.
- `PowerSnapshot`: sample timestamp in Unix milliseconds, AC-present flag, optional battery, and an independently successful/failed charge-limit reading.
- `Battery`: percentage, charging/discharging/critical flags, remaining capacity in mAh, last-full-charge capacity in mAh, design capacity in mAh, voltage in mV, and cycle count. Preserve units in field names. Do not copy manufacturer/model/serial strings into the IPC payload.
- `ChargeLimits`: minimum and maximum percentages. A successful setter reply contains the actual read-back limits, not merely the requested value.
- `PortsSnapshot`: sample timestamp and four indexed per-port results. Successful ports contain role, charging type, current/max voltage in mV, current limit/max current in mA, dual-role flag, and maximum power in mW. Keep failed ports independently unavailable.
- `InputDeckSnapshot`: sample timestamp, supported deck-state enum, and touchpad-present flag. Do not expose Laptop 16 slot layouts on the Laptop 13 or claim full USB/module inventory.

Use snake_case Rust/JSON fields and matching explicit TypeScript interfaces. Validate percentage bounds, enum conversions, message size (64 KiB maximum), and protocol compatibility in Rust before returning typed values to the UI. Unknown mandatory shapes or incompatible protocol versions produce an upgrade/compatibility error, not guessed readings. Additive fields within protocol major 1 may be ignored.

Initially unimplemented methods return `unsupported_feature` without accessing hardware; expose feature names only as implementations become available.

### Authorization and privilege boundary

- GUI and Flatpak run as the desktop user. Only the host service runs as root.
- D-Bus ownership policy permits only root to own the service name. Call policy permits the intended interface; the service remains responsible for per-method authorization.
- Polkit read action: `io.github.singh_gur.fwpanel.read-status`; active local sessions `yes`, inactive and other sessions `no`.
- Polkit write action: `io.github.singh_gur.fwpanel.set-charge-limit`; active local sessions `auth_admin`, inactive and other sessions `no`. Do not use retained `*_keep` authorization.
- Obtain the caller's unique bus name from the actual message header. Pass it as a `system-bus-name` subject to `CheckAuthorization`; never accept a claimed UID, PID, bus name, or authorization flag from client arguments.
- Authorize every status request with interaction flags zero. Failed or unavailable polkit checks fail closed and do not access hardware.
- A write first validates input and checks active-local read access, then performs the write-action check with `AllowUserInteraction`. Only the explicit Apply action initiates that call. Recheck caller presence and active-local read access before hardware mutation after the prompt completes.
- Cancel pending authorization if the caller disappears. Bound authorization waiting to 120 seconds using polkit cancellation IDs; cancellation, denial, timeout, and service errors must never dispatch the write.
- Permit at most one pending write authorization/operation globally. Return `busy` for competing writes; do not let an authentication prompt hold the hardware-read lock.
- Exclude raw data/serial numbers from IPC and logs. Do not install permissive local polkit rules, use `sudo`, launch the GUI through `pkexec`, or accept a user-selected helper path.

### Hardware execution and recovery

- Keep blocking library calls off both the GUI thread and the service's async executor. Serialize hardware operations with a single non-queuing gate; return `busy` instead of accumulating requests.
- Bound an active hardware operation to five seconds. A timed-out `spawn_blocking` task is not cancelled: mark the operation failed/uncertain and terminate the service process rather than releasing the gate and starting more hardware work beside a stuck call. Systemd may restart the service, but never replay a request.
- Treat worker panic as service failure and terminate/restart rather than reusing possibly inconsistent hardware state. Do not rely on `catch_unwind` when release panic behavior is aborting.
- Use bounded systemd restart behavior (`Restart=on-failure`, two-second restart delay, no more than three starts in 60 seconds). The GUI offers explicit Retry when automatic recovery stops.
- Set client read deadlines to ten seconds and write deadlines to 150 seconds to accommodate the bounded authentication flow. On write disconnect/deadline failure, report uncertain outcome and require readback after reconnect; never silently replay the mutation.
- An uninterruptible kernel operation cannot be made absolutely cancellable by this application. Keep UI deadlines and process/restart bounds, and report the condition rather than claiming guaranteed hardware cancellation.

### User-visible behavior

- Refresh immediately on display, then five seconds after the preceding refresh finishes. Pause periodic refresh while the document is hidden; perform a fresh read when visible again. Clear timers on component teardown.
- One refresh coordinator fetches card data sequentially to respect the service's single-operation gate. A manual refresh joins/skips an existing refresh rather than creating overlapping work.
- Each card owns loading, ready, unavailable/error, and stale states. A previous valid reading may remain visible only with a stale label and last-success timestamp. Never turn a read error into zero charge, absent battery, or disconnected port.
- Charge input accepts integer maxima from 25 through 100. Read and preserve the current minimum; reject invalid/unknown minimum values and requests below the current minimum. Do not change hysteresis or minimum thresholds implicitly.
- Apply requires deliberate interaction, is disabled during a pending write, and does not optimistically report success. Success requires readback equal to the requested maximum with the minimum preserved.
- No automatic setting changes on launch, refresh, retry, shutdown, suspend/resume, or reboot. Do not promise that firmware retains the limit across reboot until separately verified.
- Ports describe USB-C power/charging information, not guaranteed identification of every expansion card. Use upstream's Laptop 13 mapping: 0 right rear, 1 right front, 2 left front, 3 left rear; verify it physically before shipping labels.
- Use native accessible controls, visible keyboard focus, semantic labels/status messages, and light/dark CSS. No tray, background controller, telemetry, network feature, history database, or update service.

## Assumptions and Bounded Discovery

- The owner can approve system-service installation and test hardware on this laptop. Until then, mark privileged checks **Not Run**, not passed.
- API presence is established, but useful behavior through the kernel EC driver is unverified. Phase 2 is the decisive hardware checkpoint. If required APIs do not work, stop with the exact operation/error and seek a plan amendment; do not add port-I/O/CLI fallback or duplicate hardware logic.
- Build/install tooling may need owner-approved installation. No task authorizes reading secrets, sensitive local configuration, or credential stores.
- Phase 7 must select and pin a currently supported GNOME runtime/SDK providing the WebKitGTK API required by Tauri, plus compatible Node 24 and Rust SDK extensions. Verify published metadata and installed tooling; do not copy the old runtime numbers in documentation examples blindly.
- The Flatpak node-source generator documents pnpm support. Actual offline compatibility with this pnpm 12 lock/store format remains a bounded Phase 7 check. Failure requires a reported blocker, not switching package managers or enabling network during builds.
- Flathub submission, signing/publication infrastructure, and non-Fedora host-service packages are outside this plan.

## Phase Strategy

- Dependency order is sequential: **1 -> 2 -> 3 -> 4 -> 5 -> 6 -> 7**.
- Do not parallelize writers across shared contracts or the current working directory. This plan gains more from reviewed checkpoints than concurrent implementation.
- Estimates are rough focused implementation time, excluding package downloads, installation approvals, review, and unexpected hardware investigation.
- Start every phase as `Not Started`, change to `In Progress` when work begins, and check off only completed tasks. Strike through skipped/superseded tasks with the reason; never mark them completed.
- Mark a phase `Complete` only after verification evidence and explicit owner confirmation. Any consequential architecture, security, scope, or compatibility change requires a plan amendment and approval.

### Version-control checkpoints

- Create `feat/initial-01-service`, `feat/initial-02-power`, through `feat/initial-07-flatpak` from the accepted result of the preceding phase.
- Start with a clean checkout; preserve unrelated owner changes rather than stashing/resetting them automatically.
- Use conventional commits. Do not merge or publish merely because tests passed; obtain the owner's phase acceptance and merge approval.
- After an accepted merge, update the plan's phase tracking and remove the completed branch only through the agreed owner workflow. Start dependent work from the merged result, not an unfinished sibling branch.

## Phase 1 — Service and Permission Boundary

- **Objective:** establish a separately privileged, authenticated, inspectable service and typed GUI connection without hardware access.
- **Status:** Complete (owner-accepted 2026-09-10; merged to main).
- **Complexity:** High
- **Estimated Time:** 90–150 minutes
- **Prerequisites:** approved plan; owner approval before any system installation.
- **Context:** this replaces the documented CLI architecture and establishes the contracts all later phases consume.

### Files

| Path | Action | Purpose |
| --- | --- | --- |
| `Cargo.toml`, `Cargo.lock` | Create | Workspace containing `src-tauri` and the two new Rust crates; unified resolved dependencies |
| `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` | Modify / retire child lockfile | Add protocol/D-Bus client dependencies and use the workspace lockfile |
| `crates/fwpanel-protocol/Cargo.toml`, `src/lib.rs` | Create | Shared wire DTOs, reply envelope, protocol version, input validation and tests |
| `crates/fwpanel-service/Cargo.toml`, `src/main.rs`, `src/auth.rs` | Create | D-Bus service entry point and polkit checks |
| `src-tauri/src/lib.rs`, `src-tauri/src/service.rs` | Modify / create | Typed service client and `get_service_info` Tauri command |
| `packaging/systemd/fwpanel-service.service` | Create | Root system service, absolute executable path, bounded restart policy |
| `packaging/dbus/io.github.singh_gur.Fwpanel1.service`, `io.github.singh_gur.Fwpanel1.conf` | Create | System-bus activation and ownership/call policy |
| `packaging/polkit/io.github.singh_gur.fwpanel.policy` | Create | Read and write action definitions |
| `packaging/stage-service.sh`, `packaging/check-service.sh` | Create | Unprivileged staging and explicit non-mutating service checks |
| `justfile`, `.gitignore`, `README.md`, `AGENTS.md`, `vite.config.js` | Modify | Workspace commands/output paths, staging instructions, architecture and watcher alignment |

### Implementation Tasks

- [x] Create a Cargo workspace with resolver 2 and explicit member paths. Carry the existing release optimization settings into the workspace root, because member profiles are ignored. Generate the root lockfile with Cargo, preserve existing resolutions where compatible, and remove the now-unused child lockfile only once workspace checks succeed. (Child `src-tauri/Cargo.lock` retired after `cargo check/test/clippy` passed.)
- [x] Add dependencies using Cargo commands; keep `framework_lib` confined to the service crate when added in Phase 2. The protocol crate uses serde/serde_json only. Use zbus's Tokio integration in the service; keep client blocking work off Tauri's GUI thread. (Service: zbus 5 with `tokio` feature, no defaults. GUI: zbus 5 defaults incl. `blocking-api`; blocking calls wrapped in `tauri::async_runtime::spawn_blocking`.)
- [x] Implement the approved protocol, including round-trip tests, malformed/oversized/incompatible reply rejection, and integer charge-limit validation. Do not introduce a general schema/type-generation system. (12 tests in `fwpanel-protocol`.)
- [x] Implement all named D-Bus methods. Only `GetServiceInfo` returns real data in this phase; the remaining methods authorize as appropriate but return `unsupported_feature` before hardware access or write authentication. Advertise no hardware features yet. (`ServiceInfo.features` is empty; pinned by a unit test.)
- [x] Implement polkit checks using the message's real sender and the specified action defaults. Validate authorization result shape and deny on any error. Keep policy test seams private; no production flag or installed fake backend may bypass authorization. (All `Err(_) => false` fail-closed; unit tests pin action ids and the `(sa{sv})` subject signature.)
- [x] Implement `get_service_info() -> Result<ServiceInfo, String>` and map absent service, denied access, invalid reply, and version mismatch to clear errors. Keep the existing greeting UI until Phase 3; register the new command through the existing Tauri wiring.
- [x] Installable service paths are `/usr/libexec/fwpanel-service`, `/usr/lib/systemd/system/fwpanel-service.service`, `/usr/share/dbus-1/system-services/io.github.singh_gur.Fwpanel1.service`, `/usr/share/dbus-1/system.d/io.github.singh_gur.Fwpanel1.conf`, and `/usr/share/polkit-1/actions/io.github.singh_gur.fwpanel.policy`. Use root-owned, non-user-writable installed files; never execute the service as root directly from the checkout. (Paths wired in `packaging/stage-service.sh`; staged 0755/0644. Nothing installed.)
- [x] Add `Type=dbus` and the approved bus name to the systemd unit; use D-Bus activation rather than enabling a polling daemon at boot. Start with `NoNewPrivileges=yes`, `ProtectSystem=strict`, `ProtectHome=yes`, and `PrivateTmp=yes`; do not use `PrivateDevices=yes`, which would hide the required EC device. Additional restrictions require hardware verification and must not break the approved driver silently. (Unit is not enabled; `WantedBy` present for optional enablement only.)
- [x] Add `just stage-service <destdir>` to build and stage the executable/policies under a caller-owned directory, without escalation or writes to system paths. Document the exact file installation/removal procedure for an owner-operated administrator session. Service installation remains a separate, explicit owner action. (Verified: staged to `/tmp/fwpanel-stage` with correct modes.)
- [x] Add `just check-service` for read-only D-Bus introspection/service-info checks. Verify policy defaults and sender handling with small Rust tests plus real denied-access checks where an inactive/remote test session is available; do not manufacture successful evidence when it is not. (Script added; Rust tests cover action ids/subject shape. Installed-service denial checks **Not Run** — service not installed.)
- [x] Update existing docs to describe the new architecture, Node 24/pnpm baseline, no CLI requirement, separate privilege boundary, and workspace build paths. Ignore root `target/`, packaging staging/build outputs, and Rust/Flatpak artifacts. Update Vite's ignored paths for new Rust crates. Do not mark later features implemented.

### Execution Tracking Rules

Apply the global tracking rules. Record staged/installed paths and whether an owner actually installed the service; a staged directory is not an installed-system test.

### Verification

- [x] `cargo fmt --all -- --check`, `cargo check --workspace`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings` succeed. (2026-09-10; 17 tests pass.)
- [x] `pnpm check` and `pnpm build` succeed with no change to the SvelteKit static/SSR decisions. (0 errors/warnings.)
- [x] `systemd-analyze verify packaging/systemd/fwpanel-service.service` succeeds after the referenced executable is staged/installed appropriately; record any path-only staging limitation rather than suppressing errors. (Live verify reports only the missing `/usr/libexec/fwpanel-service` — expected pre-install. With `--root=/tmp/fwpanel-stage` the unit parses; remaining complaint is the minimal sysroot lacking system-provided `dbus.socket`, not this unit. Staged smoke run: unprivileged binary reaches the system bus and is correctly refused the root-only bus name.)
- [x] After owner installation: `busctl --system introspect io.github.singh_gur.Fwpanel1 /io/github/singh_gur/Fwpanel1` lists only the intended application methods plus standard D-Bus interfaces. (2026-09-10: exactly GetServiceInfo/GetPower/GetPorts/GetInputDeck/SetChargeLimit(u)→s plus Introspectable/Peer/Properties.)
- [x] `busctl --system call io.github.singh_gur.Fwpanel1 /io/github/singh_gur/Fwpanel1 io.github.singh_gur.Fwpanel1 GetServiceInfo` returns protocol-1 service information to the active desktop user without a prompt. (2026-09-10: `{"status":"ok","protocol_version":1,...,"features":[]}`. Also verified: GetPower and SetChargeLimit 80 return `unsupported_feature` with no authentication prompt; polkit EnumerateActions shows read-status 0/0/5 (no/no/authorized) and set-charge-limit 0/0/2 (no/no/auth_admin), no retained variants; service D-Bus-activated and running under the hardened unit.)
- [x] Read-access denial and polkit-unavailable conditions do not reach a hardware function. All unimplemented methods return explicit unavailable/unsupported results. (Live denial: unit-test level only — no inactive/remote session was available, so the live denial check is **Not Run** per the no-manufactured-evidence rule. Live positive evidence: all four unimplemented methods authorize silently and return `unsupported_feature` without hardware access or write prompts.)

### Completion Gate

Owner confirms the service/authorization boundary, installation state, and verification evidence. Do not begin privileged hardware integration while this boundary remains unverified.

### Outputs

Working service-info round trip, testable authorization/protocol code, staged/installed service assets, workspace commands, and aligned project guidance.

## Phase 2 — Live Battery and Charge-Limit Reads

- **Objective:** prove the library/kernel-driver path on the target laptop and return validated battery/AC data.
- **Status:** Complete (owner-accepted 2026-09-10; merged to main).
- **Complexity:** High
- **Estimated Time:** 60–120 minutes
- **Prerequisites:** Phase 1 accepted; owner-installed service and permission to perform the selected read-only checks.
- **Context:** API availability alone is not hardware compatibility. This is a stop/go checkpoint before investing in the full UI.

### Files

| Path | Action | Purpose |
| --- | --- | --- |
| `crates/fwpanel-service/Cargo.toml`, `src/main.rs`, `src/hardware.rs` | Modify / create | Pinned library, platform/driver selection, serialized and bounded hardware operations |
| `crates/fwpanel-protocol/src/lib.rs` | Modify | Complete power conversions/validation if required by verified values |
| `src-tauri/src/lib.rs`, `src-tauri/src/service.rs` | Modify | `get_power` command and typed decoding |
| `packaging/check-service.sh`, `README.md`, `AGENTS.md` | Modify | Read-only checks and proven hardware support/limitations |

### Implementation Tasks

- [ ] Add `framework_lib@=0.6.5` with `--no-default-features` using Cargo in the service crate. Confirm the compiled source matches the researched API; do not substitute unreleased `main` APIs.
- [ ] Detect the platform with `framework_lib::smbios::get_platform`; enable hardware operations only for `Platform::Framework13AmdAi300`. Select `CrosEc::with(CrosEcDriverType::CrosEc)` and return distinct unsupported-platform/driver errors.
- [ ] Implement the single non-queuing hardware gate, off-executor blocking execution, five-second operation deadline, and panic/timeout process-failure policy. Do not create overlapping workers when a call times out.
- [ ] Implement `GetPower` using `power::power_info(&ec)` and `ec.get_charge_limit()`. Map `None` to unavailable, not absent battery; only a successful `PowerInfo` with `battery: None` means no battery. Preserve charge-limit failure separately so it does not hide otherwise valid battery data.
- [ ] Validate all mapped percentages and units; reject inconsistent/impossible required values instead of clamping them. Keep upstream serial/manufacturer/model data out of replies and application logs. Do not call print-oriented CLI/library wrappers.
- [ ] Add `get_power() -> Result<PowerSnapshot, String>` to Tauri and register it. Advertise battery and charge-limit-read features only after implementation.
- [ ] Add small tests for absent battery, valid charging/discharging states, failed optional charge-limit reads, malformed values, timeout, busy, and panic/disconnect recovery using test-only closures/data. Do not require physical hardware for `cargo test`.
- [x] Reinstall the staged service only with owner approval; compare status with visible battery behavior while plugging/unplugging AC. If upstream panics or the selected driver cannot read the target, stop and record exact evidence; do not silently patch upstream or change the driver boundary. (Owner reinstalled 2026-09-10; no upstream panic; driver read works.)

### Execution Tracking Rules

Apply global rules. Record real hardware results separately from synthetic tests and distinguish each feature's unsupported/failed status.

### Verification

- [x] Run the workspace Rust checks and `pnpm check`/`pnpm build` from Phase 1; all pass. (2026-09-10; 29 tests, clippy -D warnings clean.)
- [x] `busctl --system call io.github.singh_gur.Fwpanel1 /io/github/singh_gur/Fwpanel1 io.github.singh_gur.Fwpanel1 GetPower` returns validated status without authentication prompts for the active local user. (2026-09-10: on-battery reading — ac_present false, 54% discharging, 2577/4763 mAh consistent with percentage, cycle count 16, limits 0/100.)
- [x] AC transitions and battery percentage are plausible on the target; charge-limit reads agree with their returned contract. No setting is changed. (2026-09-10: after plugging AC — ac_present true, charging true, discharging false, voltage 15393→15951 mV, limits stable; both power states observed; reads only.)
- [x] Missing driver, unsupported platform, read failure, service death, timeout, and busy conditions are explicit in tests/manual evidence; none become valid zero/absent readings. (Unit tests: unavailable-not-absent-battery, sentinel limits → Failed, invalid percentage → invalid_data, gate busy/timeout/panic; service-death UX is Phase 3.)
- [x] Failure tests demonstrate a bounded client response and no concurrent second hardware call beside a hung worker. (gate_timeout_leaves_gate_engaged + gate_is_non_queuing; 10s client deadline.)

### Completion Gate

Owner confirms live read functionality and accepts any explicitly reported upstream limitations. Required battery support failure blocks dependent feature work pending a plan amendment.

### Outputs

Verified read-only integration, safe data conversion, service recovery behavior, and typed power command.

## Phase 3 — Usable Dashboard

- **Objective:** replace the template with a responsive battery dashboard and clear service/error states.
- **Status:** Complete (owner-accepted 2026-09-10; merged to main).
- **Complexity:** Medium
- **Estimated Time:** 60–90 minutes
- **Prerequisites:** Phase 2 accepted.
- **Context:** keep page-local Svelte 5 state; extract card components only when they have actual independent markup/behavior.

### Files

| Path | Action | Purpose |
| --- | --- | --- |
| `src/routes/+page.svelte`, `src/lib/types.ts` | Replace / create | Dashboard, typed DTOs, refresh/error states |
| `src/lib/components/BatteryCard.svelte` | Create if extracted | Battery presentation and accessible status markup |
| `src-tauri/src/lib.rs`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json` | Modify | Remove greet, configure CSP, trim template permissions |
| `package.json`, `pnpm-lock.yaml`, `src-tauri/Cargo.toml`, `Cargo.lock` | Update through package managers | Remove unused opener dependencies if no approved feature uses them |
| `static/vite.svg`, `static/tauri.svg`, `static/svelte.svg` | Delete | Unused template branding |
| `README.md`, `AGENTS.md` | Modify | Implemented UI and refresh/verification guidance |

### Implementation Tasks

- [ ] Replace the greeting form/logos and remove `greet` from Rust registration. Present application/service status plus battery percentage, AC/charging state, capacity/cycle details, and the read-only current charge limit.
- [ ] Mirror protocol DTOs in explicit strict TypeScript types. Use `invoke<T>` and Svelte runes; no `any`, legacy stores, raw JSON parsing, or browser-side hardware access.
- [ ] Implement the approved sequential five-second refresh coordinator, manual Retry/Refresh, pause-on-hidden, immediate visible refresh, and teardown cleanup. Distinguish missing service from incompatible service and failed hardware reads.
- [ ] Implement per-card loading/unavailable/stale states and last-success timestamps. Preserve independent charge-limit failure without erasing battery status. Prevent a response arriving after teardown from updating a destroyed view.
- [ ] Use native semantic controls, visible focus, non-color-only errors, accessible status announcements without announcing every ordinary poll, light/dark styling, and an 800x600 usable layout with sensible resizing.
- [ ] Configure a minimal Tauri-compatible production CSP using the installed Tauri documentation/schema; allow only required local assets and IPC. Use narrowly scoped development allowances for Vite/HMR. No remote script loading or disabling CSP to make a check pass.
- [ ] Remove opener permissions/plugin dependencies through pnpm/Cargo if still unused. Do not add broad Tauri shell/filesystem permissions; custom Rust command registration is not itself a reason to add unrelated plugin capabilities.
- [ ] Keep UI tests lightweight: Rust covers reply/data failures; execute and record the concrete desktop scenarios below. Do not add a browser-test framework for this phase.

### Execution Tracking Rules

Apply global rules. Record desktop smoke tests separately from a successful frontend build; a browser preview does not verify Tauri IPC.

### Verification

- [ ] `pnpm check`, `pnpm build`, and all workspace Rust checks pass.
- [ ] `pnpm tauri dev` displays live battery data; hide/show the window and navigate keyboard controls. Confirm no overlapping refreshes or polling prompts.
- [ ] Stop/uninstall the test service only with owner approval: the GUI remains responsive, marks previous data stale, and provides a useful Retry/install message. Restore service and verify recovery.
- [ ] Test malformed/incompatible responses through test-only client fixtures, plus no-battery and failed-limit states. Nothing is presented as successful data accidentally.
- [ ] Light/dark modes, keyboard focus, resizing, and production asset/CSP loading work.

### Completion Gate

Owner reviews the real desktop UI and confirms its live and failure behavior.

### Outputs

Usable read-only dashboard, removed scaffold assets, minimal application permissions, and documented smoke checks.

## Phase 4 — Authenticated Charge-Limit Control

- **Objective:** safely apply and verify a maximum charge limit through a narrow authorized operation.
- **Status:** Complete (owner-accepted 2026-09-10; merged to main).
- **Complexity:** High
- **Estimated Time:** 60–120 minutes
- **Prerequisites:** Phase 3 accepted; separate owner approval for a reversible real hardware write test.
- **Context:** this is the only hardware-setting operation in the initial release.

### Files

| Path | Action | Purpose |
| --- | --- | --- |
| `crates/fwpanel-service/src/main.rs`, `src/auth.rs`, `src/hardware.rs` | Modify | Authorized read/validate/write/readback sequence |
| `crates/fwpanel-protocol/src/lib.rs` | Modify | Complete mutation validation and result tests |
| `src-tauri/src/lib.rs`, `src-tauri/src/service.rs` | Modify | `set_charge_limit` typed command and write deadline |
| `src/routes/+page.svelte`, `src/lib/components/ChargeLimitControl.svelte`, `src/lib/types.ts` | Modify / create | Native input, Apply, pending/error/uncertain-result UI |
| `README.md`, `AGENTS.md` | Modify | Write boundary and explicit hardware-test procedure |

### Implementation Tasks

- [ ] Implement `set_charge_limit(maximum: u32) -> Result<ChargeLimits, String>` in Tauri. Validate integers/range in the UI and shared Rust validation, then validate independently at the service trust boundary before any mutation.
- [ ] Activate the specified write polkit flow, global pending-write guard, caller-disappearance cancellation, and bounded auth timeout. A denied/cancelled/error result never reaches `set_charge_limit`.
- [ ] Under the hardware gate, read current `(min, max)`. Reject invalid limits, unknown/sentinel minimums outside 0–100, and requested maxima below the current minimum. If already equal, return verified current values without an unnecessary EC write.
- [ ] Call upstream `ec.set_charge_limit(current_minimum, requested_maximum)` once, then `ec.get_charge_limit()`. Success requires matching requested maximum and preserved minimum. A failed/mismatched readback is not success and must report uncertain/failed verification.
- [ ] Do not retry mutation automatically after timeout, disconnect, mismatch, denial, or reboot. Recover by a fresh read; if the result remains unknown, require an explicit new owner action rather than claiming rollback.
- [ ] Add a native number input (25–100, step 1), explicit Apply, pending authorization/applying state, readable errors, and disabled competing submissions. Refresh the displayed actual setting after completion without overwriting an unrelated unsaved draft during ordinary polls.
- [ ] Add small tests proving range rejection, minimum preservation, conflicting minimum rejection, no write on denied/cancelled auth, one write at most, no-op behavior, readback mismatch, and uncertain-result handling. Use a small private callback seam for ordered calls; do not introduce a public hardware-provider abstraction.

### Execution Tracking Rules

Apply global rules. Record original/requested/read-back/restored limits for owner-approved hardware testing without storing unrelated hardware identifiers.

### Verification

- [x] All frontend/workspace checks pass; validation and authorization tests prove rejected paths do not call hardware mutation. (2026-09-10: 39 tests; gate_after_auth seam proves denied/cancelled/timed-out auth and eligibility loss never reach hardware.)
- [x] Manual authentication cancellation leaves the original setting unchanged; reads/polling never trigger authentication dialogs. (2026-09-10: owner cancelled one prompt → access_denied, setting unchanged; every one of 6+ writes prompted individually — no retained/temporary auto-approval observed; polls never prompted.)
- [x] With owner approval, record the original setting, apply a distinct safe permitted maximum, verify actual readback, and restore the original value. (2026-09-10: original 0/100 → applied 80 with verified readback 0/80 (minimum preserved) → restored 100 with verified readback 0/100. Restoration succeeded.)
- [x] A client disconnect/service failure produces uncertain status and no automatic replay. (Dismissed/timed-out prompt run: client gone during auth → denied, no write dispatched — the disappearance path live-verified; ChargeLimitOps tests prove no automatic retry; the UI re-polls (fresh read) after any failure before reporting the actual setting.)
- [x] Startup/refresh/reopen cause no write and make no unverified reboot-persistence claim. (Read paths contain no write calls; no persistence claims in UI or docs.)

### Completion Gate

Owner confirms authentication, write/readback, and restoration evidence. Unit tests alone do not complete the hardware-write phase.

### Outputs

One narrow authenticated control, validation tests, verified readback behavior, and a reversible hardware-test record.

## Phase 5 — USB-C and Input-Deck Status

- **Objective:** complete the approved read-only hardware cards without expanding into unsupported module inventory or controls.
- **Status:** Complete (owner-accepted 2026-09-10; merged to main).
- **Complexity:** Medium
- **Estimated Time:** 60–90 minutes
- **Prerequisites:** Phase 4 accepted.
- **Context:** USB-C PD power state and input-deck state are different features and fail independently.

### Files

| Path | Action | Purpose |
| --- | --- | --- |
| `crates/fwpanel-service/src/hardware.rs`, `src/main.rs` | Modify | Library-backed port/deck queries and feature advertisement |
| `crates/fwpanel-protocol/src/lib.rs` | Modify | Port/deck conversion tests |
| `src-tauri/src/lib.rs`, `src-tauri/src/service.rs` | Modify | `get_ports` and `get_input_deck` commands |
| `src/lib/components/PortsCard.svelte`, `InputDeckCard.svelte` | Create | Independent status cards |
| `src/routes/+page.svelte`, `src/lib/types.ts` | Modify | Sequential refresh integration and matching types |
| `README.md`, `AGENTS.md` | Modify | Actual supported data and limitations |

### Implementation Tasks

- [ ] Implement `GetPorts` using `power::get_pd_info(&ec, 4)`. Preserve per-port `EcResult` failures and map upstream roles/types explicitly; verify source units rather than inferring them from variable names.
- [ ] Implement `GetInputDeck` using `ec.get_input_deck_status()`, exposing deck state and touchpad presence only. Map a specifically unsupported command/version to `unsupported_feature`; generic I/O failures remain hardware errors, not absence.
- [ ] Route both through the existing read authorization and hardware gate/deadline. Add typed Tauri wrappers and protocol conversions; do not create alternate privileged access paths.
- [ ] Integrate cards into the single sequential refresh coordinator. One failed port or card must not suppress the rest of the dashboard.
- [ ] Use the approved four-port labels only after a physical charger-movement test verifies mapping. Until verified, use port indices rather than confident incorrect labels.
- [ ] Add conversion tests for disconnected vs failed ports, roles, unknown/invalid values, and unsupported deck data. Handle upstream panic by the established service-failure mechanism, not by invented default deck state.
- [ ] If typed upstream APIs cannot supply a selected field safely, report the limitation for owner review. Do not parse print functions, reproduce board-specific ADC/EC logic, or add input-deck power controls.

### Execution Tracking Rules

Apply global rules. Record supported, unavailable, and unverified fields explicitly; do not mark unsupported features as hardware successes.

### Verification

- [x] All frontend/workspace checks pass. (2026-09-10: 44 tests, clippy/fmt, pnpm check/build, dev smoke.)
- [x] On the target, move the charger among available ports and verify indexed/physical mapping and sensible voltage/current units. (2026-09-10: charger verified on index 0 = right rear after owner moved it; index 3 = left rear was the original charger position with a verified 60 W PD contract (20 V/3 A) after the µW→mW unit fix from upstream power.rs:978. Ports 1/2 occupied by HDMI (right front) and USB-A (left front) cards: port 1 reads source/5 V (laptop powering the HDMI card), port 2 disconnected — consistent with upstream mapping but not charger-verified, so the UI keeps plain indices.)
- [x] Verify deck/touchpad status where supported without opening the powered laptop or removing internal components for a test. (2026-09-10: deck_state=on, touchpad_present=true on the target.)
- [x] Simulated per-port/deck failures preserve battery and charge-limit functionality and display unavailable/stale status accurately. (Unit tests: failed port → Unavailable while others stay ok; deck unsupported vs hardware errors; per-card UI snapshots keep prior data with last-success timestamps.)

### Completion Gate

Owner accepts the additional cards and their exact compatibility/availability limits.

### Outputs

Four USB-C power-status entries, supported deck/touchpad information, and tested independent failure behavior.

## Phase 6 — Fedora RPM Packaging

- **Objective:** provide reproducible, maintainable installation of the trusted service and native GUI on Fedora 44.
- **Status:** Complete pending owner acceptance (2026-09-10).
- **Complexity:** Medium
- **Estimated Time:** 90–150 minutes
- **Prerequisites:** Phase 5 accepted; owner-approved RPM build tools and test installation environment.
- **Context:** the service-only package is also a prerequisite for Flatpak users. Do not bundle root execution into the portable GUI.

### Files

| Path | Action | Purpose |
| --- | --- | --- |
| `packaging/rpm/fwpanel-service.spec` | Create | Helper-only RPM using existing staged assets |
| `src-tauri/tauri.conf.json` | Modify | Explicit native RPM target and service package dependency |
| `packaging/io.github.singh-gur.fwpanel.metainfo.xml`, desktop/icon assets | Create / modify | Application identity and package metadata |
| `justfile`, `packaging/check-service.sh`, `.gitignore` | Modify | Repeatable build/check recipes and ignored outputs |
| `README.md`, `AGENTS.md` | Modify | Service/GUI package lifecycle, paths, tooling and compatibility |

### Implementation Tasks

- [ ] Build `fwpanel-service` from the workspace release output and package only the host executable, systemd unit, D-Bus policy/activation file, polkit actions, and required licensing metadata. Use root ownership, mode 0755 for the executable, and 0644 for policy/unit files.
- [ ] Use Fedora's standard systemd RPM scriptlet macros and verify their available syntax through installed packaging documentation. Package installation must not modify hardware, start polling, grant blanket authorization, or make the service enabled at boot unnecessarily.
- [ ] Configure Tauri's native GUI bundle target as RPM only; remove `targets: all`. Require the service's compatible protocol package capability (for example an RPM `Provides: fwpanel-service-api = 1` and matching GUI `Requires`), plus real runtime dependencies. Do not require the official CLI.
- [ ] Give the GUI and service packages separate ownership of their files. The GUI package does not own or remove the service's policy files. Keep the existing application ID `io.github.singh-gur.fwpanel` for desktop/Flatpak identity; the D-Bus name uses underscores as specified earlier.
- [ ] Add `just build-service-rpm` and `just build-rpm`; the latter builds service and GUI deliverables with locked dependencies. Verify workspace `target/release/bundle/` output locations rather than retaining stale `src-tauri/target` assumptions.
- [ ] Add existing-license declarations and third-party notices required by dependency licensing. Replace template-facing desktop metadata/icons with fwpanel identity; do not invent certification, official endorsement, screenshots, or release URLs.
- [ ] Test clean install, upgrade, explicit service removal, GUI-only removal, and service reinstallation on an owner-approved Fedora environment. Preserve normal package-manager dependency behavior; never bypass dependency protection to make an uninstall test pass.
- [ ] Update docs with exact GUI/service prerequisites, artifact paths, package commands, tested hardware scope, and failure guidance. Do not claim broad Linux support from one Fedora test.

### Execution Tracking Rules

Apply global rules. Keep staging/build outputs untracked. Record actual package names/versions, inspection commands, and lifecycle results; do not publish artifacts as part of this phase.

### Verification

- [x] Frontend/workspace checks and release builds pass with locked dependencies. (2026-09-10.)
- [x] `rpm -qlp <service.rpm>`, `rpm -qp --requires <service.rpm>`, and `rpm -qp --scripts <service.rpm>` show the intended paths, runtime requirements, and bounded lifecycle actions; perform equivalent GUI package checks. (Service: 6 files, provides fwpanel-service-api-1 + fwpanel-service-api = 1, reload-only scriptlets. GUI: binary/desktop/icons/metainfo, requires fwpanel-service-api-1 + webkit2gtk4.1 + gtk3.)
- [x] `just build-service-rpm` and `just build-rpm` produce installable x86_64 artifacts. (Built via a local createrepo_c test repo; dnf5 @commandline resolution ignores rpmdb/repo provides for local files — noted below.)
- [x] After owner-approved installation, launch the native GUI from the desktop menu, not only a terminal. (2026-09-10: owner installed both RPMs from the local test repo with --nogpgcheck (unsigned local packages; signing infrastructure out of scope) and confirmed the desktop-launched GUI "works fine".)
- [x] Upgrade keeps the service/GUI protocol compatible; explicit incompatible/missing service is handled clearly. GUI-only removal behavior: owner amendment 2026-09-10 — the owner wants GUI removal to also remove the (dependency-installed) service, which is dnf5's standard orphan-dependency cleanup; the GUI package itself owns no service files. (Upgrade verified: service 0.1.0-1 → 0.1.0-2 with scriptlets. Explicitly installed services survive GUI removal per dnf5 reasons-tracking.)
- [x] Uninstall/reinstall leaves no stale locally copied policy/unit files from Phase 1 developer staging. (Owner removed the Phase 1 staging files before the RPM install; RPMs own the installed files.)

### Completion Gate

Owner confirms native package install/upgrade/removal and desktop behavior. Producing an RPM file alone is not completion.

### Outputs

Native GUI RPM, helper-only RPM, verified lifecycle behavior, and accurate installation instructions.

## Phase 7 — Flatpak GUI Delivery

- **Objective:** distribute the unprivileged GUI in a sandbox while retaining the approved host-service/polkit boundary.
- **Status:** Not Started
- **Complexity:** Medium
- **Estimated Time:** 90–150 minutes
- **Prerequisites:** Phase 6 accepted; owner-approved Flatpak builder/runtime tooling; service-only RPM installed for live tests.
- **Context:** Flatpak does not install the privileged host service. Flathub submission is not part of this phase.

### Files

| Path | Action | Purpose |
| --- | --- | --- |
| `packaging/flatpak/io.github.singh-gur.fwpanel.yml` | Create | Sandboxed GUI build/install manifest |
| `packaging/flatpak/node-sources.json`, `cargo-sources.json` | Generate | Locked offline JS/Rust sources |
| `packaging/flatpak/generate-sources.sh` | Create | Repeatable upstream-generator invocation |
| `packaging/io.github.singh-gur.fwpanel.metainfo.xml`, desktop/icon assets | Modify | Shared application metadata |
| `src-tauri/src/service.rs`, `src/routes/+page.svelte` | Modify only for demonstrated integration issues | Missing-service/version guidance and sandbox-safe connection |
| `justfile`, `.gitignore`, `README.md`, `AGENTS.md` | Modify | Flatpak recipes, ignored outputs and host prerequisite guidance |

### Implementation Tasks

- [ ] Select a currently supported GNOME runtime/SDK with Tauri's required WebKitGTK, plus compatible Node 24 and Rust extensions; pin the chosen runtime branch and generator revision. Verify metadata using `flatpak remote-info` and upstream tool documentation. Stop for owner approval if required tooling/dependency compatibility cannot be established.
- [ ] Generate JS sources using upstream `flatpak-node-generator ... pnpm pnpm-lock.yaml` and Cargo sources using its companion Cargo generator against the workspace lockfile. Inspect the pinned generator's help for exact output/store options and verify pnpm 12 compatibility; do not translate the project to npm/yarn.
- [ ] Pin/provision the required pnpm executable as a build input. Install dependencies with `pnpm install --offline --frozen-lockfile`; compile Rust using locked offline sources. Dependencies may be downloaded in the source-fetch stage, never through network-enabled build commands.
- [ ] Build and install the GUI crate only into `/app`; do not build/install the privileged service as a Flatpak component. Keep host policy/systemd assets outside the sandbox package. Reuse desktop metadata/icons from the native package.
- [ ] Grant `--system-talk-name=io.github.singh_gur.Fwpanel1`, Wayland, fallback X11, and the display-related IPC/DRI permissions actually needed by Tauri/WebKit. No `--socket=system-bus`, `--device=all`, host filesystem access, host command execution, or direct EC access. Omit runtime network permission unless an actual approved local-display requirement demonstrates otherwise; no network feature is in scope.
- [ ] Show a clear message when the host service is absent or protocol-incompatible, pointing to the separately installed `fwpanel-service` RPM and documented installation procedure. Do not attempt privileged service installation from the sandbox.
- [ ] Add `just flatpak-sources`, `just build-flatpak`, and `just check-flatpak`. Build a locally installable bundle/repository from pinned sources and retain exact builder commands in the recipe.
- [ ] Verify that host polkit identifies the real sandbox connection as an active local caller. Test the actual Flatpak Apply/cancel flow; native success does not prove sandbox authorization works. Do not weaken inactive/remote authorization policy to bypass a failed test.
- [ ] Document RPM + Flatpak installation, host-service upgrades, supported architecture/distro, and the fact that this is a locally distributable Flatpak, not an accepted Flathub publication.

### Execution Tracking Rules

Apply global rules. Track generated source manifests but ignore Flatpak build directories/repositories/bundles. Record selected runtime, SDK, pnpm, and generator revisions and exact offline validation results.

### Verification

- [ ] Native frontend/workspace checks still pass; `just flatpak-sources` and `just build-flatpak` succeed with recorded, pinned tooling.
- [ ] Build from prefetched inputs with no network access during build; it must not reuse an undeclared host `node_modules`, Cargo registry, or credential configuration.
- [ ] `flatpak info --show-permissions io.github.singh-gur.fwpanel` shows only the narrow approved permissions after local installation.
- [ ] `flatpak run io.github.singh-gur.fwpanel` opens the GUI and reads live status through the service without repeated authentication prompts.
- [ ] Missing service, incompatible service, service restart, denied write, and cancelled authentication have the same honest behavior as the native GUI.
- [ ] With separate owner approval, verify a reversible Flatpak charge-limit change/readback/restoration. No sandbox escape, direct hardware permission, or host command helper is added.

### Completion Gate

Owner confirms the locally installed Flatpak's permission boundary, live behavior, and service-installation guidance. Flathub publication is not a completion condition.

### Outputs

Reproducible Flatpak GUI artifact/manifest, offline dependency sources, narrow sandbox permissions, and tested host-service interoperability.

## Phase Dependencies

| Phase | Depends on | Required accepted output |
| --- | --- | --- |
| 1 | Plan approval | Scope and architecture decisions |
| 2 | 1 | Installed/tested authorization boundary and protocol |
| 3 | 2 | Verified battery/charge-limit-read contract |
| 4 | 3 | Usable dashboard and recoverable connection states |
| 5 | 4 | Accepted control behavior and shared refresh/error handling |
| 6 | 5 | Complete selected hardware feature set |
| 7 | 6 | Independently installable host service and native lifecycle evidence |

## Risks and Mitigations

- **Library support differs from CLI/API appearance:** Phase 2 verifies the pinned release and kernel driver on the actual laptop. Stop rather than invent a fallback.
- **Upstream panic/blocking behavior:** isolate hardware in the service; serialize work, bound client waits, terminate failed workers' service process, and limit restart loops. Never equate a future timeout with cancellation.
- **Mutation uncertainty:** validate at both trust boundaries, authorize the real bus sender, preserve the minimum, read back, and never replay automatically. Report failed restoration immediately.
- **Privilege escalation surface:** fixed service methods and paths, root-only bus ownership, narrow polkit actions, no caller-claimed identity, no arbitrary commands, no blanket device access, and authorization denial tests.
- **Multiple package/service versions:** shared protocol major and RPM compatibility capability; reject incompatible replies and give upgrade guidance. Keep service files in the service package.
- **Flatpak integration:** validate narrow system-bus access and real polkit behavior through the sandbox; maintain a separate host-service prerequisite. Offline pnpm/runtime incompatibility is a blocker, not permission to broaden network access or change package managers.
- **Firmware-dependent fields and persistence:** label unsupported/unknown values accurately, verify physical port mapping, and make no reboot-persistence promise. No automatic configuration daemon.
- **Build scope/tooling:** workspace output paths and lockfile migration must be verified with actual Tauri builds; owner-approved packaging tooling may be required.

## Out of Scope

AppImage; Flathub submission; CI/release publishing or signing infrastructure; additional laptop/distro support; firmware updates; direct EC/ioctl implementations; CLI fallback; fan/input-deck power controls; charge profiles or boot-time reapplication; tray/background UI; telemetry; history storage; generic plugin/provider frameworks.

## Questions for User

None blocking the approved plan. System installation, reversible hardware writes, phase acceptance, and any material deviation require the explicit approvals described above.

## Evidence References

- Repository: `README.md`, `AGENTS.md`, `justfile`, `package.json`, `src/routes/+page.svelte`, `src/routes/+layout.ts`, `svelte.config.js`, `vite.config.js`, `src-tauri/src/lib.rs`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`.
- Official upstream: https://github.com/FrameworkComputer/framework-system
- Published library/version metadata: https://crates.io/crates/framework_lib/0.6.5
- Pinned EC API: https://github.com/FrameworkComputer/framework-system/blob/v0.6.5/framework_lib/src/chromium_ec/mod.rs
- Pinned battery/port API: https://github.com/FrameworkComputer/framework-system/blob/v0.6.5/framework_lib/src/power.rs
- Pinned platform detection: https://github.com/FrameworkComputer/framework-system/blob/v0.6.5/framework_lib/src/smbios.rs
- Pinned CLI validation precedent: https://github.com/FrameworkComputer/framework-system/blob/v0.6.5/framework_lib/src/commandline/mod.rs (`handle_charge_limit`, 25–100%, preserve current minimum).
- Polkit authority contract: https://polkit.pages.freedesktop.org/polkit/eggdbus-interface-org.freedesktop.PolicyKit1.Authority.html
- Polkit policy behavior: https://polkit.pages.freedesktop.org/polkit/polkit.8.html
- zbus service APIs: https://z-galaxy.github.io/zbus/service.html
- Tauri RPM configuration: https://v2.tauri.app/distribute/rpm/
- Tauri Flatpak guidance: https://v2.tauri.app/distribute/flatpak/ (verify current runtime/tooling; do not copy outdated example versions).
- Flatpak permissions: https://docs.flatpak.org/en/latest/sandbox-permissions.html
- Upstream offline node/pnpm source generator: https://github.com/flatpak/flatpak-builder-tools/tree/master/node

### Execution Tracking (2026-09-10)

- Initially staged to `/tmp/fwpanel-stage` for unprivileged verification.
- Owner staged to `~/fwpanel-stage` and installed all five files in an admin
  session on 2026-09-10; installed-service checks were then run and recorded
  above. Removal procedure: README "Host service staging and installation".

## Progress

- [x] Phase 1 — Service and Permission Boundary — owner-accepted 2026-09-10; implemented on `feat/initial-01-service` (6f049b3, fixes 2dab858 + ff469ad, docs bd03ebc/7f7f7ea/d6f6b98), merged to main; verification: fmt/check/test/clippy + pnpm check/build, 18 tests, systemd-analyze (limitations recorded), staged smoke run, installed-service busctl introspection (5 methods only) + GetServiceInfo protocol-1 no-prompt + unsupported_feature paths no-prompt + polkit defaults verified live; live read-denial Not Run (no inactive/remote session available); review rounds 1–2 (2 blockers fixed, round-2 PASS); executor: root (zai/glm-5.3, session default); research: root-owned (researcher child lacked web tools); run: n/a (root-built)
- [x] Phase 2 — Live Battery and Charge-Limit Reads — owner-accepted 2026-09-10; (23c12b7); review: GATE PASS, no blockers (reviewer zai/glm-5.3/high, run 5e30a80b); live verification 2026-09-10: GetPower on target (on-battery + AC states plausible, percentage internally consistent, limits 0/100, prompts never shown); executor: root (zai/glm-5.3, session default)
- [ ] Phase 2 — Live Battery and Charge-Limit Reads
- [x] Phase 3 — Usable Dashboard — owner-accepted 2026-09-10 (manual checklist confirmed); implemented on `feat/initial-03-dashboard` (adca37d, fixes c39487a/f72f2aa); review round 1 FAIL (2 blockers: pending read shown as no-battery, service loss hid stale data) → fixed → round 2 PASS (runs 2a53edf0, e20d4258); pnpm check/build, workspace cargo, dev smoke, release-binary smoke all green; owner manual desktop checklist confirmed; executor: root (zai/glm-5.3, session default)
- [x] Phase 4 — Authenticated Charge-Limit Control — owner-accepted 2026-09-10; implemented on `feat/initial-04-charge-limit` (46128e2 + review fixes b494dd5); review: GATE PASS (run 106fa571; N1 denial-tests seam and N2 dead-code fixes applied); live verification 2026-09-10 with owner approval: original 0/100 recorded → 80 applied+verified → 100 restored+verified; cancelled prompt → access_denied with setting unchanged; every write prompted individually; executor: root (zai/glm-5.3, session default)
- [x] Phase 5 — USB-C and Input-Deck Status — owner-accepted 2026-09-10; implemented on `feat/initial-05-ports-deck` (f20cba9 + unit fix bd5d824); review: root-performed PASS (owner-approved due to reviewer-lane 429 rate limit, run 539c1f61 failed; non-fresh review recorded); live verification 2026-09-10: GetPorts all 4 ports, charger movement 3→0 verified right-rear anchor, 60 W contract units correct, deck on + touchpad present; executor: root (zai/glm-5.3, session default)
- [ ] Phase 6 — Fedora RPM Packaging — implementation complete on `feat/initial-06-rpm` (a39e6dd + capability fix d91f8a8 + docs); live lifecycle verified 2026-09-10: clean install via local test repo (dnf5 @commandline provide-resolution quirk documented; unsigned local packages need --nogpgcheck), desktop-menu launch confirmed by owner, service upgrade 0.1.0-1→0.1.0-2 with scriptlets, GUI removal removes dep-installed service (owner-preferred, dnf5 orphan cleanup, GUI owns no service files); executor: root (zai/glm-5.3, session default)
- [ ] Phase 7 — Flatpak GUI Delivery
