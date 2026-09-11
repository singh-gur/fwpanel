<script lang="ts">
  import Badge from "$lib/components/Badge.svelte";
  import Card from "$lib/components/Card.svelte";
  import Gauge from "$lib/components/Gauge.svelte";
  import Skeleton from "$lib/components/Skeleton.svelte";
  import { healthPercent, humanError, int, time, volts } from "$lib/format";
  import type { PowerSnapshot } from "$lib/types";
  import type { Tone } from "$lib/ui";

  let {
    power,
    powerError,
    lastSuccessAt,
  }: {
    power: PowerSnapshot | null;
    powerError: string | null;
    lastSuccessAt: Date | null;
  } = $props();

  const battery = $derived(power?.battery ?? null);
  // A failed read keeps the previous reading on screen, labelled stale, rather
  // than replacing real data with zeros.
  const stale = $derived(powerError !== null && power !== null);

  const limit = $derived(
    power?.charge_limit.status === "ok" ? power.charge_limit.limits.maximum_percent : null,
  );

  const tone = $derived<Tone>(
    battery === null
      ? "neutral"
      : battery.critical
        ? "danger"
        : battery.percentage <= 20
          ? "warn"
          : "ok",
  );

  const flow = $derived(
    battery === null
      ? ""
      : battery.charging
        ? "Charging"
        : battery.discharging
          ? "On battery"
          : power?.ac_present
            ? "Holding"
            : "Idle",
  );

  const sampledAt = $derived(power ? new Date(power.timestamp_ms) : null);

  const health = $derived(
    battery
      ? healthPercent(battery.last_full_charge_capacity_mah, battery.design_capacity_mah)
      : null,
  );
</script>

<Card title="Battery" id="battery-heading" icon="battery" span tone={powerError !== null && power === null ? "alert" : "default"}>
  {#if power === null && powerError === null}
    <Skeleton lines={3} text="Reading battery status…" />
  {:else if battery}
    {#if stale}
      <p class="notice" data-tone="warn">
        <span>
          <strong>Stale reading.</strong> Last successful update {time(lastSuccessAt)}.
          {humanError(powerError ?? "")}
        </span>
      </p>
    {/if}

    <div class="hero">
      <Gauge
        value={battery.percentage}
        {tone}
        {limit}
        dim={stale}
        label={`${battery.percentage}%`}
        sublabel={flow}
      />

      <div class="hero-side">
        <div class="chips">
          {#if power?.ac_present}
            <Badge tone="info" icon="plug">AC connected</Badge>
          {/if}
          {#if battery.charging}
            <Badge tone="ok" icon="arrow-up">Charging</Badge>
          {/if}
          {#if battery.discharging}
            <Badge tone="neutral" icon="arrow-down">Discharging</Badge>
          {/if}
          {#if battery.critical}
            <Badge tone="danger" icon="alert" strong>Critical</Badge>
          {/if}
          {#if limit !== null}
            <Badge tone="accent" icon="gauge">Limit {limit}%</Badge>
          {/if}
        </div>

        <dl class="stat-grid">
          <div><dt>Remaining</dt><dd>{int(battery.remaining_capacity_mah)} mAh</dd></div>
          <div>
            <dt>Last full charge</dt>
            <dd>{int(battery.last_full_charge_capacity_mah)} mAh</dd>
          </div>
          <div><dt>Design capacity</dt><dd>{int(battery.design_capacity_mah)} mAh</dd></div>
          <div><dt>Voltage</dt><dd>{volts(battery.voltage_mv)}</dd></div>
          <div><dt>Cycle count</dt><dd>{int(battery.cycle_count)}</dd></div>
          {#if health !== null}
            <div><dt>Health</dt><dd>{health}%</dd></div>
          {/if}
        </dl>
      </div>
    </div>

    {#if sampledAt}
      <p class="hint">Sampled {time(sampledAt)}</p>
    {/if}
  {:else if power}
    <!-- A successful read with battery: null genuinely means no battery. -->
    <p class="empty">No battery detected — running on AC.</p>
  {:else}
    <p class="notice" data-tone="danger">
      <span>
        <strong>Battery status unavailable.</strong>
        {humanError(powerError ?? "")}
      </span>
    </p>
    {#if lastSuccessAt}
      <p class="hint">Last successful read: {time(lastSuccessAt)}.</p>
    {/if}
  {/if}
</Card>

<style>
  .hero {
    display: flex;
    align-items: center;
    gap: var(--sp-6);
    flex-wrap: wrap;
  }
  .hero-side {
    flex: 1 1 20rem;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }
  /* Keep the stats to about three columns; stretched across a wide window
     they read as a scattered row rather than a block. */
  .hero-side :global(.stat-grid) {
    max-width: 34rem;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
  }
  .empty {
    margin: 0;
    color: var(--text-muted);
  }
</style>
