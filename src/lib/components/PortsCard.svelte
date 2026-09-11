<script lang="ts">
  import Badge from "$lib/components/Badge.svelte";
  import Card from "$lib/components/Card.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import Skeleton from "$lib/components/Skeleton.svelte";
  import { amps, humanError, time, voltsCompact, watts } from "$lib/format";
  import type { InputDeckSnapshot, PortRole, PortsSnapshot } from "$lib/types";
  import type { Tone } from "$lib/ui";

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
  const roleTone: Record<PortRole, Tone> = {
    disconnected: "neutral",
    source: "info",
    sink: "ok",
    sink_not_charging: "warn",
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
</script>

<Card
  title="USB-C ports"
  id="ports-heading"
  icon="plug"
  span
  tone={card.error && !results ? "alert" : "default"}
>
  {#if results}
    <ul class="ports">
      {#each results as result, i (i)}
        {@const active = result.status === "ok" && result.port.role !== "disconnected"}
        <li class="port" class:idle={!active} data-state={result.status === "ok" ? result.port.role : "error"}>
          <span class="dot" aria-hidden="true"></span>
          <span class="port-name">Port {i}</span>

          {#if result.status === "ok"}
            <Badge tone={roleTone[result.port.role] ?? "neutral"}>
              {roleLabel[result.port.role] ?? result.port.role}
            </Badge>

            {#if active}
              <!-- A source port legitimately has no charging type; printing
                   "None" there reads like a missing reading. -->
              {#if result.port.charging_type !== "none"}
                <span class="type">
                  {typeLabel[result.port.charging_type] ?? result.port.charging_type}
                </span>
              {/if}
              {#if result.port.dual_role}
                <span class="flag" title="Dual-role port">
                  <Icon name="bolt" size={11} /> Dual-role
                </span>
              {/if}
              <span class="metrics">
                <span class="figure">{voltsCompact(result.port.current_voltage_mv)}</span>
                <span class="sep">·</span>
                <span class="figure">{amps(result.port.current_limit_ma)}</span>
                <span class="max">max {watts(result.port.max_power_mw)}</span>
              </span>
            {:else}
              <span class="type">Nothing attached</span>
            {/if}
          {:else}
            <Badge tone="danger" icon="alert">Unavailable</Badge>
            <span class="type error">{humanError(result.message)}</span>
          {/if}
        </li>
      {/each}
    </ul>
    <p class="hint">
      Physical left/right mapping of ports 0–3 is verified only during hardware tests;
      indices follow the EC numbering.
    </p>
  {:else if card.error}
    <p class="notice" data-tone="danger">
      <Icon name="alert" size={15} />
      <span><strong>USB-C status unavailable.</strong> {humanError(card.error)}</span>
    </p>
    {#if card.lastSuccessAt}
      <p class="hint">Last successful read: {time(card.lastSuccessAt)}.</p>
    {/if}
  {:else}
    <Skeleton lines={2} />
  {/if}
</Card>

<style>
  .ports {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .port {
    display: flex;
    align-items: center;
    gap: var(--sp-2) var(--sp-3);
    flex-wrap: wrap;
    padding: var(--sp-2) 0;
    border-bottom: 1px solid var(--border);
    min-width: 0;
  }
  .port:first-child {
    padding-top: 0;
  }
  .port:last-child {
    border-bottom: none;
    padding-bottom: 0;
  }
  /* An idle port is still listed, but recedes so live ports read first. */
  .idle {
    color: var(--text-faint);
  }

  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex: none;
    background: var(--row-tone, var(--idle));
  }
  .idle .dot {
    background: transparent;
    box-shadow: inset 0 0 0 1.5px var(--idle);
  }
  .port[data-state="sink"] {
    --row-tone: var(--ok);
  }
  .port[data-state="source"] {
    --row-tone: var(--info);
  }
  .port[data-state="sink_not_charging"] {
    --row-tone: var(--warn);
  }
  .port[data-state="error"] {
    --row-tone: var(--danger);
  }

  .port-name {
    font-weight: 650;
    letter-spacing: -0.01em;
    min-width: 4.2rem;
  }
  .type {
    font-size: 0.82rem;
    color: var(--text-muted);
  }
  .idle .type {
    color: var(--text-faint);
  }
  .type.error {
    color: var(--danger);
  }
  .flag {
    display: inline-flex;
    align-items: center;
    gap: 0.25em;
    font-size: 0.76rem;
    color: var(--text-faint);
  }

  /* Right-aligned figures so the four rows line up as a column of numbers. */
  .metrics {
    margin-left: auto;
    display: inline-flex;
    align-items: baseline;
    gap: 0.4em;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .figure {
    font-weight: 600;
  }
  .sep {
    color: var(--text-faint);
  }
  .max {
    margin-left: 0.3em;
    font-size: 0.8rem;
    color: var(--text-muted);
  }
</style>
