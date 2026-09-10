<script lang="ts">
  import type { InputDeckSnapshot, PortResult, PortsSnapshot } from "$lib/types";

  let {
    card,
  }: {
    card: {
      data: PortsSnapshot | InputDeckSnapshot | null;
      error: string | null;
      lastSuccessAt: Date | null;
    };
  } = $props();

  const results = $derived(
    card.data && "ports" in card.data ? (card.data as PortsSnapshot).ports : null,
  );

  const roleLabel: Record<string, string> = {
    disconnected: "Disconnected",
    source: "Source",
    sink: "Sink",
    sink_not_charging: "Sink (not charging)",
  };
  const typeLabel: Record<string, string> = {
    none: "None",
    pd: "USB-PD",
    type_c: "Type-C",
    proprietary: "Proprietary",
    bc12_dcp: "BC1.2 DCP",
    bc12_cdp: "BC1.2 CDP",
    bc12_sdp: "BC1.2 SDP",
    other: "Other",
    vbus: "VBus",
    unknown: "Unknown",
  };

  function mA(a: number): string {
    return a >= 1000 ? `${a / 1000} A` : `${a} mA`;
  }
  function mV(v: number): string {
    return v >= 1000 ? `${v / 1000} V` : `${v} mV`;
  }
  function mW(w: number): string {
    return w >= 1000 ? `${w / 1000} W` : `${w} mW`;
  }
</script>

<section class="card" aria-labelledby="ports-heading">
  <h2 id="ports-heading">USB-C ports</h2>
  {#if results}
    <ul class="ports">
      {#each results as result, i (i)}
        <li>
          <span class="port-name">Port {i}</span>
          {#if result.status === "ok"}
            <span class="chip">{roleLabel[result.port.role] ?? result.port.role}</span>
            <span class="meta">
              {typeLabel[result.port.charging_type] ?? result.port.charging_type}
              {#if result.port.role !== "disconnected"}
                · {mV(result.port.current_voltage_mv)} / {mA(result.port.current_limit_ma)} ·
                max {mW(result.port.max_power_mw)}{#if result.port.dual_role} · dual-role{/if}
              {/if}
            </span>
          {:else}
            <span class="error-text">Unavailable</span>
            <span class="meta">{result.message}</span>
          {/if}
        </li>
      {/each}
    </ul>
    <p class="hint">
      Physical left/right mapping of ports 0–3 is verified only during hardware
      tests; indices follow the EC numbering.
    </p>
  {:else if card.error}
    <p>
      <strong>USB-C status unavailable.</strong>
      <span class="error-text">{card.error}</span>
    </p>
    {#if card.lastSuccessAt}
      <p class="hint">Last successful read: {card.lastSuccessAt.toLocaleTimeString()}.</p>
    {/if}
  {:else}
    <p>Waiting for first reading…</p>
  {/if}
</section>

<style>
  .card {
    background: #fff;
    border: 1px solid #d9d9e0;
    border-radius: 12px;
    padding: 1rem 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  h2 {
    font-size: 1rem;
    margin: 0;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: #5f5f6b;
  }
  ul.ports {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
  }
  ul.ports li {
    display: flex;
    align-items: baseline;
    gap: 0.6rem;
    flex-wrap: wrap;
  }
  .port-name {
    font-weight: 600;
    min-width: 4.2rem;
  }
  .chip {
    border: 1px solid currentColor;
    border-radius: 999px;
    padding: 0.05em 0.6em;
    font-size: 0.85rem;
  }
  .error-text {
    color: #b3261e;
    font-weight: 600;
  }
  .hint,
  .meta {
    font-size: 0.85rem;
    color: #5f5f6b;
    margin: 0;
  }
  @media (prefers-color-scheme: dark) {
    .card {
      background: #2b2b33;
      border-color: #3f3f49;
    }
    .error-text {
      color: #ff8a80;
    }
  }
</style>
