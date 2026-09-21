# Implementation Plan: Sensors, Privacy Switches, and Fingerprint LED

## Overview

Add three Framework-specific features to fwpanel: a thermal/fan sensor card, a
camera/microphone privacy-switch card, and a fingerprint-LED brightness
control. Phase 1 delivers the two read-only features; Phase 2 adds the app's
second hardware write, which is also the first write that proceeds without an
administrator prompt.

This work is new scope beyond `plans/initial-development.md`. That plan's
approved phases are unchanged; its "Out of Scope" list is not contradicted by
anything here (fan *control*, input-deck controls, and charge profiles remain
out of scope — this plan only *reads* fan speed).

## Planning Profile

- Executor: Smart
- Detail: Standard
- Mode: Phased (2 phases)

## Global Context

Observed in the repository during planning; all line references verified.

- **Adding a D-Bus method touches seven files**: `crates/fwpanel-protocol/src/lib.rs`
  (DTO, validation, `feature::` const), `crates/fwpanel-service/src/hardware.rs`,
  `crates/fwpanel-service/src/main.rs`, `src-tauri/src/service.rs`,
  `src-tauri/src/lib.rs`, `src/lib/types.ts`, `src/routes/+page.svelte`, plus the
  new Svelte component.
- **A new method needs no packaging change.** `packaging/dbus/io.github.singh_gur.Fwpanel1.conf`
  allows `send_interface` for the whole interface; the systemd unit, Flatpak
  manifest, and metainfo reference only the bus name; `src-tauri/capabilities/default.json`
  contains only `core:default` and does not enumerate commands. A new **polkit
  action** does require editing `packaging/polkit/io.github.singh_gur.fwpanel.policy`.
- **Read-method shape** (`crates/fwpanel-service/src/main.rs:39-72`):
  `require_read_access(&header)` → `self.hardware.x().await` → `ok_json` /
  `error_json`. Application failures travel in the JSON envelope, never as
  D-Bus errors.
- **Write-method shape** (`crates/fwpanel-service/src/main.rs:78-141`): read
  access → trust-boundary validation → `write_pending` guard (Drop-released) →
  authorization → `gate_after_auth` → single hardware change with readback.
- **Hardware gate** (`crates/fwpanel-service/src/hardware.rs`): one non-queuing
  gate; a timed-out or panicking worker aborts the process and systemd restarts
  it. All new hardware work runs inside `gate.run(...)`.
- **Frontend card reader** (`src/routes/+page.svelte:45-57`): generic
  `readCard<T>(card, command)` storing `{ data, error, lastSuccessAt }`. A
  failed card keeps its previous data and is labelled stale rather than zeroed.
  The refresh cycle is sequential and returns early if `get_service_info` fails
  (`+page.svelte:91-97`).
- **Client timeouts** (`src-tauri/src/service.rs`): reads use `METHOD_TIMEOUT`
  (10s) through the shared `call()` helper; the write path builds its own
  connection with `WRITE_TIMEOUT` (150s) at lines 107-130.
- **Card render ladder** (`PortsCard.svelte:78-113`, `InputDeckCard.svelte:43-71`):
  data → unsupported → error with last-success time → `Skeleton`.
- **Upstream APIs** (framework_lib 0.6.5, pinned `=0.6.5`):
  - `CrosEcDriver` is public (`src/chromium_ec/mod.rs:215`) exposing `read_memory`.
  - `power::print_thermal()` only prints to stdout and returns nothing; the
    offsets and the `TempSensor` decoder are private.
  - `ec.get_privacy_info() -> EcResult<(bool, bool)>` (microphone, camera),
    command `0x3E14`.
  - `ec.get_fp_led_level() -> EcResult<(u8, Option<FpLedBrightnessLevel>)>`,
    `ec.set_fp_led_percentage(u8)` (command version 1, 1–100%), command `0x3E0E`.
    `FpLedBrightnessLevel::Custom = 0xFE` is documented get-only.

## Architecture Decisions

1. **Two new read methods, `GetSensors` and `GetPrivacy`**, not one combined
   call. Keeps DTOs domain-clean and matches the existing one-method-per-domain
   shape. Accepted cost: the 5s cycle grows from four to six sequential calls.
   Each opens a fresh system-bus connection, but the EC reads are small.
2. **Read the EC memory map directly** (owner decision). Via the public
   `CrosEcDriver` trait: `read_memory(0x00, 0x0F)` for temperatures and
   `read_memory(0x10, 0x08)` for four little-endian `u16` fan RPM entries. We
   redeclare the sentinels (`0xFF` not present, `0xFE` error, `0xFD` not
   powered, `0xFC` not calibrated; fans `0xFFFF` not present, `0xFFFE` stalled),
   the conversion `°C = raw - 73`, and the Framework 13 AMD AI 300 sensor order
   (`F75303_Local`, `F75303_CPU`, `F75303_DDR`, `APU`). All of it lives in one
   documented module carrying an explicit justification comment, because this is
   a conscious, narrow deviation from the `AGENTS.md` rule against duplicating
   EC knowledge. It is the only way to obtain structured thermals from the
   pinned release.
3. **Compute Celsius in `i16`, not `u8`.** Upstream evaluates `t - 73` on a
   `u8` (`power.rs:93`), which panics in debug builds for any raw value below
   73. Our decoder must not inherit that defect.
4. **Per-item failures use `status: "ok" | "unavailable"`**, matching
   `PortResult`. `ChargeLimitReading` uses `"failed"` for the same concept; the
   codebase is already inconsistent here. Follow the closer analogue and do not
   modify existing types.
5. **Gate new cards on `ServiceInfo.features`.** `features` is declared
   (`src/lib/types.ts:8`) and advertised by the service
   (`hardware.rs:405-413`) but consumed nowhere in the frontend. A newer GUI
   talking to an older service RPM would otherwise surface a confusing
   transport error. The new cards check the advertised feature name and render
   an "update fwpanel-service" notice instead. Scoped to the new cards only;
   existing cards are not changed.
6. **Extract a generic `CardSnapshot<T>` into `src/lib/types.ts`.** The type is
   currently written out three times (`+page.svelte:34-38`, `PortsCard.svelte:13-17`,
   `InputDeckCard.svelte:12-17`); two more cards would make it five copies plus
   five `in`-check discriminators. Migrate Ports and InputDeck onto the generic
   type so the `in`-check casts disappear.
7. **The fingerprint LED sets a percentage (1–100)** via `set_fp_led_percentage`
   (command version 1), not the coarse level enum. Finer control, and it reuses
   the existing charge-limit slider interaction. Validation must reject
   `FpLedBrightnessLevel::Custom`, which is get-only.
8. **Fingerprint LED authorization: new polkit action
   `io.github.singh_gur.fwpanel.set-fp-led` with `allow_active=yes`** (owner
   decision), `allow_inactive=no`, `allow_any=no`. LED brightness is cosmetic
   and fully reversible, so an administrator prompt per change is unjustified
   friction. The 120s bounded-auth machinery is therefore unused for this
   action, but the `write_pending` guard, double validation, single write, and
   readback verification all still apply.

## Assumptions

- "Approved" covers the two questions raised at draft review: version control
  uses a feature branch per phase (matching `plans/initial-development.md`), and
  decisions 5 and 6 are in scope. Raise an objection before Phase 1 starts if
  either was not intended.
- The target machine is Framework Laptop 13 AMD Ryzen AI 300 on Fedora 44; no
  other platform is claimed or tested.
- `PROTOCOL_MAJOR` stays at `1`. Adding methods and DTOs is additive: unknown
  fields are ignored by `Reply::decode`, and new method names do not break an
  older client. The RPM's `Provides: fwpanel-service-api-1` therefore does not
  change.

## Phase Strategy

Phase 1 groups the two read-only features: no new polkit action, no hardware
writes, no owner approval needed for live verification. Phase 2 isolates the
single privileged write, which needs a policy change, packaging text updates,
and owner-approved live hardware testing. Splitting there means a problem with
the write cannot block shipping the reads.

---

## Phase 1 — Sensors and Privacy Reads

- **Objective:** Serve `GetSensors` and `GetPrivacy` and render both as cards.
- **Status:** Not Started
- **Complexity:** Medium
- **Estimated Time:** 60–90 minutes
- **Prerequisites:** None

### Context for this Phase

Both features are pure reads behind the existing `read-status` polkit action,
so active local users see no prompt. The only novel element is the EC memory-map
decoder (decision 2), which is also this phase's main risk: a wrong offset or
conversion produces plausible-looking but wrong numbers, so verification must
compare against an independent source.

### Files

| Path | Action | Purpose |
| --- | --- | --- |
| `crates/fwpanel-protocol/src/lib.rs` | modify | Sensor/privacy DTOs, validation, `feature::` consts, tests |
| `crates/fwpanel-service/src/hardware.rs` | modify | `ec_memmap` module, `sensors()`, `privacy()`, `implemented_features()` |
| `crates/fwpanel-service/src/main.rs` | modify | `GetSensors` / `GetPrivacy` interface methods |
| `src-tauri/src/service.rs` | modify | Proxy methods and typed wrappers |
| `src-tauri/src/lib.rs` | modify | Tauri commands and `generate_handler!` list |
| `src/lib/types.ts` | modify | DTO mirrors, generic `CardSnapshot<T>` |
| `src/lib/ui.ts`, `src/lib/components/Icon.svelte` | modify | New icon names and glyphs |
| `src/routes/+page.svelte` | modify | Card state, poll cycle, layout |
| `src/lib/components/PortsCard.svelte`, `InputDeckCard.svelte` | modify | Migrate to generic `CardSnapshot<T>` |
| `src/lib/components/SensorsCard.svelte` | create | Temperature and fan card |
| `src/lib/components/PrivacyCard.svelte` | create | Microphone/camera switch card |

### Implementation Tasks

- [ ] Add `TempReading` and `FanReading` as `status: "ok" | "unavailable"`
      unions (decision 4), plus `SensorsSnapshot { timestamp_ms, temperatures,
      fans }` and `PrivacySnapshot { timestamp_ms, microphone_enabled,
      camera_enabled }`. Give each a `validated()` that rejects implausible
      values rather than clamping, following `Battery::validated`.
- [ ] Add `feature::SENSORS` and `feature::PRIVACY` and register both in
      `hardware::implemented_features()`.
- [ ] Add an `ec_memmap` module in `hardware.rs` holding the offsets,
      sentinels, sensor labels, and the `i16`-safe Celsius conversion
      (decisions 2 and 3), with a comment justifying the duplication and
      pointing at the upstream source it mirrors. Unit-test every sentinel, a
      normal value, and a raw value below 73.
- [ ] Add `HardwareAccess::sensors()` and `HardwareAccess::privacy()`, each a
      single `gate.run(...)` closure. Map `EcError::Response(InvalidCommand |
      InvalidVersion)` from `get_privacy_info()` to `ErrorCode::UnsupportedFeature`,
      matching `build_input_deck_snapshot`; other errors stay
      `HardwareUnavailable`.
- [ ] Wire both methods through `main.rs`, the `service.rs` proxy and wrappers
      (shared `call()` helper, 10s read timeout), the Tauri commands, and
      `generate_handler!`.
- [ ] Extract generic `CardSnapshot<T>` into `types.ts` and migrate Ports and
      InputDeck onto it, removing the `in`-check casts (decision 6).
- [ ] Add TS mirrors for the new DTOs, plus the new card state and the two
      `readCard` calls in the refresh cycle.
- [ ] Add the icons the new cards need to both `ui.ts` and `Icon.svelte` — the
      current set is `battery, bolt, plug, gauge, keyboard, refresh, check,
      alert, shield, chip, arrow-down, arrow-up`.
- [ ] Build `SensorsCard.svelte` and `PrivacyCard.svelte` on the existing render
      ladder, using `Card`/`Badge`/`Icon` and the shared `.notice` / `.hint`
      classes. No inline `style` attributes — the production CSP blocks them
      silently. Add the `features`-based unsupported notice (decision 5).

### Execution Tracking Rules

Set status to `In Progress` when work starts. Check off tasks as completed.
Strike through superseded tasks rather than marking them done. Only mark the
phase `Complete` after the owner confirms.

### Verification

- [ ] `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`,
      `cargo test --workspace` — all clean.
- [ ] `pnpm check` (0 errors) and `pnpm build`.
- [ ] Live `busctl` call of `GetSensors` and `GetPrivacy` on the target: both
      return plausible data with **no authentication prompt**.
- [ ] **Cross-check temperatures against kernel hwmon (`sensors`) and fan RPM
      against an independent source.** This is the objective test that the
      redeclared offsets and conversion are correct and must pass before the
      phase is accepted.
- [ ] Toggle the physical privacy switches and confirm the card follows within
      one refresh cycle.
- [ ] Confirm a failing card keeps stale data with a last-success time and does
      not suppress the other cards.

### Completion Gate

Owner reviews the running dashboard and explicitly confirms the phase is done.

### Outputs

Two new D-Bus methods, two new cards, a generic `CardSnapshot<T>`, a tested
`ec_memmap` decoder, and two new advertised features.

---

## Phase 2 — Fingerprint LED Brightness

- **Objective:** Serve `GetFpLed` / `SetFpLed` and ship a verified brightness control.
- **Status:** Not Started
- **Complexity:** Medium
- **Estimated Time:** 60–90 minutes
- **Prerequisites:** Phase 1 (shared `CardSnapshot<T>`)

### Context for this Phase

This is the app's second hardware write and the first that proceeds without an
administrator prompt (decision 8). The verification semantics depend on an
unknown — whether the EC stores the exact percentage or quantizes it to a
brightness level — which must be settled by evidence before the success rule is
coded.

### Files

| Path | Action | Purpose |
| --- | --- | --- |
| `crates/fwpanel-protocol/src/lib.rs` | modify | `FpLedState`, `FpLedLevel`, `validate_fp_led_request`, tests |
| `crates/fwpanel-service/src/auth.rs` | modify | `ACTION_SET_FP_LED` and non-interactive check |
| `crates/fwpanel-service/src/hardware.rs` | modify | `fp_led()` read and verified `set_fp_led()` write |
| `crates/fwpanel-service/src/main.rs` | modify | `GetFpLed` / `SetFpLed` interface methods |
| `src-tauri/src/service.rs`, `src-tauri/src/lib.rs` | modify | Proxy, wrappers, commands |
| `src/lib/types.ts`, `src/routes/+page.svelte` | modify | DTO mirrors and wiring |
| `src/lib/components/FpLedControl.svelte` | create | Brightness control |
| `packaging/polkit/io.github.singh_gur.fwpanel.policy` | modify | New `set-fp-led` action |
| `packaging/check-service.sh` | modify | "five application methods" text and a new smoke call |
| `packaging/rpm/fwpanel-service.spec` | modify | Changelog entry for the protocol-visible change |

### Implementation Tasks

- [ ] **Bounded probe, before coding the verification rule.** With owner
      approval, write a known percentage and read back via `get_fp_led_level()`.
      Record whether the EC returns the exact value or quantizes it.
      Decision rule: exact → require an exact match like `ChargeLimitOps`;
      quantized → success means the readback falls in the expected bucket, and
      the actual value is reported. Do not guess — a wrong rule makes every
      write report `outcome_unknown`. Report the finding and confirm the chosen
      semantics before implementing.
- [ ] Add `FpLedLevel` (`high`, `medium`, `low`, `ultra_low`, `custom`, `auto`),
      `FpLedState { percentage, level: Option<FpLedLevel> }`, and
      `validate_fp_led_request` accepting 1–100 and rejecting `custom`
      (decision 7). Add `feature::FP_LED_READ` / `FP_LED_WRITE` and register them.
- [ ] Add `ACTION_SET_FP_LED` plus a non-interactive polkit check (flags `0`),
      keeping the existing fail-closed behavior.
- [ ] Add `HardwareAccess::fp_led()` and `set_fp_led()`. The write follows the
      `ChargeLimitOps` seam: read current → validate → write once → verify by
      readback → never retry automatically. Handle the v1→v0 fallback and the
      `DeviceError` that `get_fp_led_level` raises for an unknown level value.
- [ ] Add `GetFpLed` and `SetFpLed` to the service, reusing the `set_charge_limit`
      skeleton minus the prompt: read access → validation → `write_pending`
      guard → authorization → hardware change. Client-side, the write reuses the
      bespoke connection pattern at `service.rs:107-130`.
- [ ] Add the polkit action with `allow_any=no`, `allow_inactive=no`,
      `allow_active=yes`, and a comment stating why this write does not prompt.
- [ ] Build `FpLedControl.svelte` on the `ChargeLimitControl` pattern (draft
      state synced only while not dirty, `canApply`, verified result notice,
      `onApplied` triggering a parent refresh), **without** the administrator-prompt
      notice.
- [ ] Update the "five application methods" text in `check-service.sh`, add a
      `GetFpLed` smoke call, and add the spec changelog entry.

### Execution Tracking Rules

Same as Phase 1. The probe task must be resolved and its outcome recorded in
this file before the verification rule is implemented.

### Verification

- [ ] Full Rust and frontend gates as in Phase 1.
- [ ] Live apply on the target with owner approval: the LED visibly changes and
      the readback verification reports success.
- [ ] **No password prompt** appears for the active local user.
- [ ] An inactive or remote session is denied.
- [ ] A rejected value (0, 101, `custom`) fails validation with
      `invalid_argument` and performs no write.
- [ ] Original brightness restored after testing.

### Completion Gate

Owner confirms the control works on hardware and the authorization behavior is
what they intended.

### Outputs

Two new D-Bus methods, a new polkit action, a new control, and a recorded
finding about EC readback fidelity.

---

## Phase Dependencies

Phase 1 → Phase 2 (shared `CardSnapshot<T>`). Phase 1 is independently
shippable; Phase 2 is not required for it.

## Version Control Strategy

A feature branch per phase, matching `plans/initial-development.md`:
`feat/sensors-privacy` then `feat/fp-led`. Start a phase by branching from an
up-to-date `main`. When the owner confirms a phase complete, merge to `main`
and delete the branch before starting the next, so no unfinished phase state is
left behind. Phase 2 branches from `main` after Phase 1 merges.

## Risks

- **Redeclared EC layout drifts from firmware.** Mitigated by the hwmon
  cross-check and by isolating every constant in one module. If a future
  framework_lib exposes structured sensor data, replace the module with it.
- **Fingerprint LED readback may quantize**, breaking exact-match verification.
  Mitigated by the bounded probe gating the verification rule.
- **The authorization model widens**: the first fwpanel write that proceeds
  without a prompt. Cosmetic and reversible, but state it plainly in the policy
  comment and the commit message.
- **Live hardware writes need explicit owner approval** per `AGENTS.md`; Phase 2
  cannot be fully verified without it.
- **Poll cycle grows to six (Phase 1) then seven (Phase 2) sequential calls.**
  Acceptable at this count; if an eighth is added, revisit whether reads should
  be batched or run on staggered cadences.

## Questions for User

None blocking. The two draft-review questions are recorded under Assumptions;
correct them before Phase 1 begins if the blanket approval was not intended to
cover them.
