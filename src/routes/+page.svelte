<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import ChargeLimitControl from "$lib/components/ChargeLimitControl.svelte";
  import InputDeckCard from "$lib/components/InputDeckCard.svelte";
  import PortsCard from "$lib/components/PortsCard.svelte";
  import type { InputDeckSnapshot, PortsSnapshot, PowerSnapshot, ServiceInfo } from "$lib/types";

  // Service connection state. Kinds map to the stable "service-*" prefixes
  // produced by src-tauri/src/service.rs.
  type ServiceState =
    | { kind: "connecting" }
    | { kind: "ok"; info: ServiceInfo }
    | { kind: "unavailable" }
    | { kind: "denied" }
    | { kind: "incompatible" }
    | { kind: "error"; message: string };

  let serviceState: ServiceState = $state({ kind: "connecting" });
  let power = $state<PowerSnapshot | null>(null);
  // Non-null while the latest power read failed; the previous reading (if any)
  // stays visible and is labeled stale instead of being replaced by zeros.
  let powerError: string | null = $state(null);
  let lastSuccessAt = $state<Date | null>(null);
  let refreshing = $state(false);
  let announcement = $state("");

  // Per-card snapshots for the secondary read-only cards. A failed card keeps
  // its previous data (stale) or an unavailable message; one failed card does
  // not suppress the others.
  interface CardSnapshot {
    data: PortsSnapshot | InputDeckSnapshot | null;
    error: string | null;
    lastSuccessAt: Date | null;
  }
  function freshCard(): CardSnapshot {
    return { data: null, error: null, lastSuccessAt: null };
  }
  let portsCard = $state<CardSnapshot>(freshCard());
  let deckCard = $state<CardSnapshot>(freshCard());

  async function readCard<T extends PortsSnapshot | InputDeckSnapshot>(
    card: CardSnapshot,
    command: string,
  ): Promise<void> {
    try {
      const data = await guard(invoke<T>(command));
      card.data = data;
      card.error = null;
      card.lastSuccessAt = new Date();
    } catch (e) {
      if (disposed) return;
      card.error = String(e);
    }
  }

  const REFRESH_INTERVAL_MS = 5000;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let disposed = false;
  let announcedKind = "connecting";

  function classify(detail: string): ServiceState {
    if (detail.startsWith("service-unavailable")) return { kind: "unavailable" };
    if (detail.startsWith("service-denied")) return { kind: "denied" };
    if (
      detail.startsWith("service-incompatible") ||
      detail.startsWith("service-invalid-reply")
    ) {
      return { kind: "incompatible" };
    }
    return { kind: "error", message: detail };
  }

  async function guard<T>(call: Promise<T>): Promise<T> {
    const result = await call;
    // A response that arrives after teardown must not touch destroyed state.
    if (disposed) throw new Error("component teardown");
    return result;
  }

  // One refresh cycle: sequential calls (the service serializes hardware
  // reads anyway), service info first so its failure colors everything.
  async function refreshOnce(): Promise<void> {
    if (refreshing) return; // manual refresh joins/skips an in-flight one
    refreshing = true;
    try {
      try {
        const info = await guard(invoke<ServiceInfo>("get_service_info"));
        serviceState = { kind: "ok", info };
      } catch (e) {
        if (disposed) return;
        serviceState = classify(String(e));
        powerError = "fwpanel service problem — data not refreshed.";
        return;
      }
      try {
        const snapshot = await guard(invoke<PowerSnapshot>("get_power"));
        power = snapshot;
        powerError = null;
        lastSuccessAt = new Date();
      } catch (e) {
        if (disposed) return;
        powerError = String(e);
      }
      // One failed card must not suppress the rest.
      await readCard<PortsSnapshot>(portsCard, "get_ports");
      await readCard<InputDeckSnapshot>(deckCard, "get_input_deck");
    } finally {
      refreshing = false;
    }
  }

  // Schedule the next poll five seconds after this refresh finishes; never
  // while the document is hidden.
  async function refreshCycle(): Promise<void> {
    await refreshOnce();
    if (!disposed && !document.hidden) {
      clearTimeout(timer);
      timer = setTimeout(() => void refreshCycle(), REFRESH_INTERVAL_MS);
    }
  }

  function manualRefresh() {
    void refreshCycle();
  }

  $effect(() => {
    disposed = false;
    void refreshCycle();
    const onVisibility = () => {
      if (document.hidden) {
        clearTimeout(timer); // pause periodic refresh while hidden
      } else {
        void refreshCycle(); // fresh read as soon as we are visible again
      }
    };
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      disposed = true;
      document.removeEventListener("visibilitychange", onVisibility);
      clearTimeout(timer);
    };
  });

  // Announce state transitions only — not every poll.
  $effect(() => {
    const kind = serviceState.kind;
    if (kind !== announcedKind) {
      announcedKind = kind;
      announcement =
        kind === "ok"
          ? "fwpanel service connected"
          : kind === "connecting"
            ? "Connecting to the fwpanel service"
            : "fwpanel service problem: " + serviceStatusLabel(kind);
    }
  });

  function serviceStatusLabel(kind: string): string {
    switch (kind) {
      case "ok":
        return "Connected";
      case "connecting":
        return "Connecting…";
      case "unavailable":
        return "Not installed or not running";
      case "denied":
        return "Access denied for this session";
      case "incompatible":
        return "Incompatible service version";
      default:
        return "Service error";
    }
  }

  const batteryStale = $derived(powerError !== null && power !== null);

  function timeLabel(date: Date | null): string {
    return date ? date.toLocaleTimeString() : "";
  }

  const battery = $derived(power?.battery ?? null);
  const chargeLimit = $derived(power?.charge_limit ?? null);
</script>

<main>
  <header>
    <h1>fwpanel</h1>
    <p class="service-status" data-state={serviceState.kind}>
      <span class="state">{serviceStatusLabel(serviceState.kind)}</span>
      {#if serviceState.kind === "ok"}
        <span class="meta">
          service {serviceState.info.service_version} · library
          {serviceState.info.library_version}
        </span>
      {:else if serviceState.kind === "unavailable"}
        <span class="meta">install fwpanel-service, then press Retry</span>
      {:else if serviceState.kind === "incompatible"}
        <span class="meta">update fwpanel-service or the app</span>
      {:else if serviceState.kind === "error"}
        <span class="meta">{serviceState.message}</span>
      {/if}
    </p>
    <button type="button" onclick={manualRefresh} disabled={refreshing}>
      {refreshing ? "Refreshing…" : "Retry"}
    </button>
  </header>

  <p class="announcement" role="status" aria-live="polite">{announcement}</p>

  {#if power === null && powerError === null}
    <section class="card" aria-labelledby="battery-heading">
      <h2 id="battery-heading">Battery</h2>
      <p>Reading battery status…</p>
    </section>
  {:else if battery}
    <section class="card" aria-labelledby="battery-heading">
      <h2 id="battery-heading">Battery</h2>
      {#if batteryStale}
        <p class="stale-note">
          <strong>Stale reading</strong> — last successful update
          {timeLabel(lastSuccessAt)}. {powerError}
        </p>
      {/if}
      <p class="charge">
        <span class="percent">{battery.percentage}%</span>
        <span class="state-chips">
          {#if power?.ac_present}<span class="chip">AC connected</span>{/if}
          {#if battery.charging}<span class="chip">Charging</span>{/if}
          {#if battery.discharging}<span class="chip">Discharging</span>{/if}
          {#if battery.critical}<span class="chip chip-critical">Critical</span>{/if}
        </span>
      </p>
      <dl class="details">
        <div><dt>Remaining</dt><dd>{battery.remaining_capacity_mah} mAh</dd></div>
        <div><dt>Last full charge</dt><dd>{battery.last_full_charge_capacity_mah} mAh</dd></div>
        <div><dt>Design capacity</dt><dd>{battery.design_capacity_mah} mAh</dd></div>
        <div><dt>Voltage</dt><dd>{battery.voltage_mv / 1000} V</dd></div>
        <div><dt>Cycle count</dt><dd>{battery.cycle_count}</dd></div>
      </dl>
      <p class="hint">Sampled {timeLabel(new Date(power!.timestamp_ms))}</p>
    </section>
  {:else if power}
    <!-- A successful read with battery: null genuinely means no battery. -->
    <section class="card" aria-labelledby="battery-heading">
      <h2 id="battery-heading">Battery</h2>
      <p>No battery detected (running on AC).</p>
    </section>
  {:else}
    <section class="card error" aria-labelledby="battery-heading">
      <h2 id="battery-heading">Battery</h2>
      <p>
        <strong>Battery status unavailable.</strong>
        {powerError ?? ""}
      </p>
      <p class="hint">
        {#if lastSuccessAt}Last successful read: {timeLabel(lastSuccessAt)}.{/if}
      </p>
    </section>
  {/if}

  <ChargeLimitControl {chargeLimit} onApplied={() => void refreshCycle()} />

  <PortsCard card={portsCard} />

  <InputDeckCard card={deckCard} />

  <footer>
    <span class="meta">
      {#if lastSuccessAt}Last successful read: {timeLabel(lastSuccessAt)}{/if}
    </span>
  </footer>
</main>

<style>
  :global(:root) {
    font-family: system-ui, sans-serif;
    font-size: 15px;
    color-scheme: light dark;
  }
  :global(body) {
    margin: 0;
    background: #f6f6f8;
    color: #1a1a1f;
  }
  @media (prefers-color-scheme: dark) {
    :global(body) {
      background: #232329;
      color: #eeeef2;
    }
  }

  main {
    max-width: 720px;
    margin: 0 auto;
    padding: 1rem 1.25rem 2rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  header {
    display: flex;
    align-items: center;
    gap: 1rem;
    flex-wrap: wrap;
  }
  h1 {
    font-size: 1.4rem;
    margin: 0;
  }
  .service-status {
    flex: 1;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-width: 12rem;
  }
  .service-status .state {
    font-weight: 600;
  }
  .service-status[data-state="ok"] .state::before {
    content: "● ";
  }
  .service-status[data-state="unavailable"] .state,
  .service-status[data-state="denied"] .state,
  .service-status[data-state="incompatible"] .state,
  .service-status[data-state="error"] .state {
    color: #b3261e;
  }
  @media (prefers-color-scheme: dark) {
    .service-status[data-state="unavailable"] .state,
    .service-status[data-state="denied"] .state,
    .service-status[data-state="incompatible"] .state,
    .service-status[data-state="error"] .state {
      color: #ff8a80;
    }
  }

  button {
    font: inherit;
    padding: 0.45em 1.2em;
    border-radius: 8px;
    border: 1px solid #7a7a85;
    background: #fff;
    color: inherit;
    cursor: pointer;
  }
  button:disabled {
    opacity: 0.6;
    cursor: default;
  }
  @media (prefers-color-scheme: dark) {
    button {
      background: #2e2e36;
      border-color: #8a8a95;
    }
  }
  button:focus-visible {
    outline: 2px solid #396cd8;
    outline-offset: 2px;
  }

  .announcement {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    margin: -1px;
  }

  .card {
    background: #fff;
    border: 1px solid #d9d9e0;
    border-radius: 12px;
    padding: 1rem 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  .card.error {
    border-color: #b3261e;
  }
  @media (prefers-color-scheme: dark) {
    .card {
      background: #2b2b33;
      border-color: #3f3f49;
    }
    .card.error {
      border-color: #ff8a80;
    }
  }
  .card h2 {
    font-size: 1rem;
    margin: 0;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: #5f5f6b;
  }

  .charge {
    margin: 0;
    display: flex;
    align-items: baseline;
    gap: 0.8rem;
    flex-wrap: wrap;
  }
  .percent {
    font-size: 2.6rem;
    font-weight: 700;
    line-height: 1;
  }
  .state-chips {
    display: flex;
    gap: 0.4rem;
    flex-wrap: wrap;
  }
  .chip {
    border: 1px solid currentColor;
    border-radius: 999px;
    padding: 0.1em 0.7em;
    font-size: 0.85rem;
  }
  .chip-critical {
    font-weight: 700;
    border-width: 2px;
  }

  .stale-note {
    margin: 0;
    font-style: italic;
  }

  dl.details {
    margin: 0;
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(9rem, 1fr));
    gap: 0.5rem 1.25rem;
  }
  dl.details div {
    display: flex;
    flex-direction: column;
  }
  dl.details dt {
    font-size: 0.8rem;
    color: #5f5f6b;
  }
  dl.details dd {
    margin: 0;
    font-variant-numeric: tabular-nums;
  }

  .hint,
  .meta {
    font-size: 0.85rem;
    color: #5f5f6b;
    margin: 0;
  }
  footer {
    display: flex;
    justify-content: flex-end;
  }
</style>
