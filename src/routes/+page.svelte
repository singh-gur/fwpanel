<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import BatteryCard from "$lib/components/BatteryCard.svelte";
  import ChargeLimitControl from "$lib/components/ChargeLimitControl.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import InputDeckCard from "$lib/components/InputDeckCard.svelte";
  import PortsCard from "$lib/components/PortsCard.svelte";
  import { humanError, time } from "$lib/format";
  import type { InputDeckSnapshot, PortsSnapshot, PowerSnapshot, ServiceInfo } from "$lib/types";
  import type { Tone } from "$lib/ui";

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

  // Only a user-initiated refresh drives the button's busy state. `refreshing`
  // flips every five seconds for background polls, and letting that reach the
  // button made it flash and grey out continuously. Duplicate work is still
  // prevented by the guard at the top of refreshOnce().
  let manualRefreshing = $state(false);

  async function manualRefresh(): Promise<void> {
    if (manualRefreshing) return;
    manualRefreshing = true;
    try {
      await refreshCycle();
    } finally {
      manualRefreshing = false;
    }
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

  const statusTone = $derived.by<Tone>(() => {
    switch (serviceState.kind) {
      case "ok":
        return "ok";
      case "connecting":
        return "neutral";
      case "unavailable":
      case "incompatible":
        return "warn";
      default:
        return "danger";
    }
  });

  // Short, actionable second line under the status pill.
  const statusDetail = $derived.by(() => {
    switch (serviceState.kind) {
      case "unavailable":
        return "Install fwpanel-service, then press Retry";
      case "incompatible":
        return "Update fwpanel-service or the app";
      case "denied":
        return "Only active local sessions may read status";
      case "error":
        return humanError(serviceState.message);
      default:
        return "";
    }
  });

  const connected = $derived.by(() => serviceState.kind === "ok");
  const chargeLimit = $derived(power?.charge_limit ?? null);
</script>

<div class="app">
  <header class="topbar">
    <div class="brand">
      <svg class="mark" viewBox="0 0 32 32" width="30" height="30" aria-hidden="true">
        <rect x="1" y="1" width="30" height="30" rx="9" class="mark-bg" />
        <path
          d="M18.2 7.5 11 17.1h4.5L14.3 24.5l7.3-9.8h-4.6z"
          class="mark-glyph"
        />
      </svg>
      <div class="wordmark">
        <span class="name">fwpanel</span>
        <span class="tagline">Framework Control Panel</span>
      </div>
    </div>

    <div class="status" data-tone={statusTone}>
      <span class="status-line">
        <span class="dot" class:pulse={serviceState.kind === "connecting"}></span>
        <span class="status-label">{serviceStatusLabel(serviceState.kind)}</span>
      </span>
      {#if statusDetail}
        <span class="status-detail">{statusDetail}</span>
      {/if}
    </div>

    <button
      type="button"
      class="btn refresh"
      onclick={() => void manualRefresh()}
      disabled={manualRefreshing}
    >
      <span class="spin" class:spinning={manualRefreshing}>
        <Icon name="refresh" size={15} />
      </span>
      {manualRefreshing ? "Refreshing…" : connected ? "Refresh" : "Retry"}
    </button>
  </header>

  <p class="sr-only" role="status" aria-live="polite">{announcement}</p>

  <main class="content">
    <div class="grid">
      <BatteryCard {power} {powerError} {lastSuccessAt} />
      <ChargeLimitControl {chargeLimit} onApplied={() => void refreshCycle()} />
      <InputDeckCard card={deckCard} />
      <PortsCard card={portsCard} />
    </div>
  </main>

  <footer class="statusbar">
    <span class="meta">
      {#if lastSuccessAt}Last successful read {time(lastSuccessAt)}{:else}No reading yet{/if}
    </span>
    {#if serviceState.kind === "ok"}
      <span class="meta dim">
        service {serviceState.info.service_version} · framework_lib
        {serviceState.info.library_version}
      </span>
    {/if}
  </footer>
</div>

<style>
  .app {
    min-height: 100vh;
    display: flex;
    flex-direction: column;
    background:
      radial-gradient(90rem 40rem at 50% -12rem, var(--bg-accent), transparent 70%),
      var(--bg);
  }

  /* Top bar ---------------------------------------------------------------- */
  .topbar {
    position: sticky;
    top: 0;
    z-index: 10;
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    flex-wrap: wrap;
    padding: var(--sp-3) var(--sp-6);
    border-bottom: 1px solid var(--border);
    /* Fallback first: older WebKitGTK drops the color-mix() declaration. */
    background: var(--surface);
    background: color-mix(in srgb, var(--surface) 88%, transparent);
    -webkit-backdrop-filter: blur(12px);
    backdrop-filter: blur(12px);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    min-width: 0;
  }
  .mark {
    display: block;
    flex: none;
  }
  .mark-bg {
    fill: var(--accent);
  }
  .mark-glyph {
    fill: var(--accent-ink);
  }
  .wordmark {
    display: flex;
    flex-direction: column;
    line-height: 1.15;
    min-width: 0;
  }
  .name {
    font-size: 1.02rem;
    font-weight: 700;
    letter-spacing: -0.02em;
  }
  .tagline {
    font-size: 0.72rem;
    color: var(--text-faint);
    letter-spacing: 0.02em;
  }

  /* Service status --------------------------------------------------------- */
  .status {
    margin-left: auto;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 1px;
    min-width: 0;
    text-align: right;
  }
  .status[data-tone="ok"] {
    --tone: var(--ok);
  }
  .status[data-tone="warn"] {
    --tone: var(--warn);
  }
  .status[data-tone="danger"] {
    --tone: var(--danger);
  }
  .status[data-tone="neutral"] {
    --tone: var(--idle);
  }
  .status-line {
    display: inline-flex;
    align-items: center;
    gap: 0.45em;
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--tone);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--tone) 18%, transparent);
    flex: none;
    /* No fallback ring is needed — the dot itself carries the colour. */
  }
  .pulse {
    animation: pulse 1.4s var(--ease) infinite;
  }
  @keyframes pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.3;
    }
  }
  .status-label {
    font-size: 0.86rem;
    font-weight: 600;
    color: var(--tone);
  }
  .status-detail {
    font-size: 0.74rem;
    color: var(--text-faint);
  }

  .refresh {
    flex: none;
  }
  .spin {
    display: flex;
  }
  .spinning {
    animation: spin 900ms linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* Content ---------------------------------------------------------------- */
  .content {
    flex: 1;
    width: 100%;
    max-width: 64rem;
    margin: 0 auto;
    padding: var(--sp-6);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--sp-4);
    align-items: start;
  }
  @media (max-width: 46rem) {
    .grid {
      grid-template-columns: minmax(0, 1fr);
    }
    .content,
    .topbar {
      padding-left: var(--sp-4);
      padding-right: var(--sp-4);
    }
  }

  /* Status bar ------------------------------------------------------------- */
  .statusbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-4);
    flex-wrap: wrap;
    padding: var(--sp-2) var(--sp-6);
    border-top: 1px solid var(--border);
    background: var(--surface);
    font-size: 0.76rem;
  }
  .statusbar .meta {
    font-size: 0.76rem;
  }
  .dim {
    color: var(--text-faint);
  }
</style>
