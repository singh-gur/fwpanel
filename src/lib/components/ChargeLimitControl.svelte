<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import Badge from "$lib/components/Badge.svelte";
  import Card from "$lib/components/Card.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import Skeleton from "$lib/components/Skeleton.svelte";
  import { humanError } from "$lib/format";
  import type { ChargeLimitReading, ChargeLimits } from "$lib/types";

  let {
    chargeLimit,
    onApplied,
  }: {
    chargeLimit: ChargeLimitReading | null;
    /** Called after a verified apply; the parent refreshes actual status. */
    onApplied: () => void;
  } = $props();

  /** Hardware-accepted range, enforced again by the service. */
  const HARD_MIN = 25;
  const HARD_MAX = 100;
  const PRESETS = [60, 80, 100];

  const currentMax = $derived(
    chargeLimit?.status === "ok" ? chargeLimit.limits.maximum_percent : null,
  );
  const currentMin = $derived(
    chargeLimit?.status === "ok" ? chargeLimit.limits.minimum_percent : null,
  );

  // The draft follows polled values only while the user has not edited it,
  // so an ordinary poll never overwrites an unsaved change.
  let draft = $state<number | null>(null);
  const dirty = $derived(draft !== null && draft !== currentMax);
  $effect(() => {
    if (!dirty) draft = currentMax;
  });

  const valid = $derived(draft !== null && draft >= HARD_MIN && draft <= HARD_MAX);
  const belowMinimum = $derived(
    currentMin !== null && draft !== null && draft < currentMin,
  );

  let applying = $state(false);
  let result = $state<{ ok: boolean; text: string } | null>(null);

  const canApply = $derived(!applying && valid && !belowMinimum && dirty);

  // Bar geometry: SVG attributes only, because the production CSP forbids
  // inline style attributes.
  const fill = $derived(Math.max(0, Math.min(100, draft ?? 0)));

  function preset(value: number) {
    if (applying) return;
    draft = value;
    result = null;
  }

  function revert() {
    draft = currentMax;
    result = null;
  }

  async function apply() {
    if (applying || !valid || belowMinimum || dirty === false) return;
    const requested = draft;
    applying = true;
    result = null;
    try {
      const limits = await invoke<ChargeLimits>("set_charge_limit", {
        maximum: requested,
      });
      result = {
        ok: true,
        text: `Verified: charge limit set to ${limits.maximum_percent}% (minimum ${limits.minimum_percent}%).`,
      };
      draft = limits.maximum_percent;
      onApplied();
    } catch (e) {
      // Never assume success; the stale notice and a manual refresh show
      // the actual setting after any failure.
      result = { ok: false, text: humanError(String(e)) };
    } finally {
      applying = false;
    }
  }
</script>

<Card title="Charge limit" id="limit-heading" icon="gauge">
  {#if chargeLimit?.status === "ok"}
    <div class="readout">
      <span class="value">{chargeLimit.limits.maximum_percent}<span class="unit">%</span></span>
      <span class="label">Maximum charge</span>
      <span class="readout-min">
        <Badge tone="neutral">Minimum {chargeLimit.limits.minimum_percent}%</Badge>
      </span>
    </div>

    <svg class="bar" viewBox="0 0 100 6" preserveAspectRatio="none" aria-hidden="true">
      <rect class="bar-track" x="0" y="0" width="100" height="6" rx="3" />
      <rect class="bar-fill" x="0" y="0" width={fill} height="6" rx="3" />
      <rect class="bar-tick" x={Math.min(99.4, chargeLimit.limits.maximum_percent)} y="-1" width="0.6" height="8" />
    </svg>

    <form
      class="controls"
      onsubmit={(e) => {
        e.preventDefault();
        void apply();
      }}
    >
      <label class="slider-row" for="charge-limit-slider">
        <span class="sr-only">New maximum charge percentage</span>
        <!-- Deliberately one-way: a range input with an empty value snaps to
             the midpoint of its range, and `bind:` would write that back into
             `draft`, making the card look edited before the user touched it. -->
        <input
          id="charge-limit-slider"
          class="slider"
          type="range"
          min={HARD_MIN}
          max={HARD_MAX}
          step="1"
          value={draft ?? currentMax ?? HARD_MIN}
          disabled={applying}
          oninput={(e) => (draft = e.currentTarget.valueAsNumber)}
        />
      </label>

      <div class="row">
        <div class="presets" role="group" aria-label="Charge limit presets">
          {#each PRESETS as value (value)}
            <button
              type="button"
              class="btn btn-sm btn-preset"
              aria-pressed={draft === value}
              disabled={applying}
              onclick={() => preset(value)}
            >
              {value}%
            </button>
          {/each}
        </div>

        <label class="numeric" for="charge-limit-input">
          <span class="sr-only">New maximum (%)</span>
          <input
            id="charge-limit-input"
            class="field"
            type="number"
            inputmode="numeric"
            min={HARD_MIN}
            max={HARD_MAX}
            step="1"
            bind:value={draft}
            disabled={applying}
          />
        </label>

        {#if dirty && !applying}
          <button type="button" class="btn btn-sm" onclick={revert}>Cancel</button>
        {/if}
        <button type="submit" class="btn btn-primary" disabled={!canApply}>
          {#if applying}
            <Icon name="refresh" size={15} />
            Applying…
          {:else}
            <Icon name="check" size={15} />
            Apply
          {/if}
        </button>
      </div>
    </form>

    {#if belowMinimum}
      <p class="notice" data-tone="danger">
        <Icon name="alert" size={15} />
        <span>
          The requested maximum is below the current minimum ({currentMin}%); raise it
          above the minimum first.
        </span>
      </p>
    {/if}
    {#if applying}
      <p class="notice" data-tone="info">
        <Icon name="shield" size={15} />
        <span>
          Administrator authorization may be requested; this can take up to two minutes.
          No setting changes until you approve it.
        </span>
      </p>
    {/if}
    {#if result}
      <p class="notice" data-tone={result.ok ? "ok" : "danger"} role="status">
        <Icon name={result.ok ? "check" : "alert"} size={15} />
        <span>{result.text}</span>
      </p>
    {/if}
    <p class="hint">
      Applied only after explicit Apply, verified by reading the setting back.
    </p>
  {:else if chargeLimit?.status === "failed"}
    <p class="notice" data-tone="danger">
      <Icon name="alert" size={15} />
      <span>
        <strong>Charge-limit reading unavailable.</strong>
        {humanError(chargeLimit.message)}
      </span>
    </p>
  {:else}
    <Skeleton lines={3} />
  {/if}
</Card>

<style>
  .readout {
    display: flex;
    align-items: baseline;
    gap: var(--sp-3);
    flex-wrap: wrap;
  }
  .value {
    font-size: 2.4rem;
    font-weight: 650;
    line-height: 1;
    letter-spacing: -0.03em;
    font-variant-numeric: tabular-nums;
  }
  .unit {
    font-size: 1.2rem;
    font-weight: 600;
    color: var(--text-muted);
    margin-left: 1px;
  }
  .readout-min {
    margin-left: auto;
    align-self: center;
  }
  .label {
    font-size: 0.72rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-faint);
  }

  .bar {
    width: 100%;
    height: 6px;
    display: block;
    overflow: visible;
  }
  .bar-track {
    fill: var(--surface-3);
  }
  .bar-fill {
    fill: var(--accent);
    transition: width 300ms var(--ease);
  }
  .bar-tick {
    fill: var(--text-faint);
  }

  .controls {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    margin: 0;
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    flex-wrap: wrap;
  }
  .presets {
    display: flex;
    gap: var(--sp-1);
  }
  /* Pushes the numeric field and the Apply pair to the right of the presets. */
  .numeric {
    margin-left: auto;
  }
  .numeric input {
    width: 5.5em;
    text-align: right;
  }

  .slider-row {
    display: block;
  }
  .slider {
    -webkit-appearance: none;
    appearance: none;
    width: 100%;
    height: 22px;
    background: transparent;
    cursor: pointer;
    margin: 0;
  }
  .slider:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .slider::-webkit-slider-runnable-track {
    height: 4px;
    border-radius: var(--r-full);
    background: var(--surface-3);
  }
  .slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 16px;
    height: 16px;
    margin-top: -6px;
    border-radius: 50%;
    background: var(--surface);
    border: 2px solid var(--accent);
    box-shadow: var(--shadow-sm);
    transition: transform var(--dur) var(--ease);
  }
  .slider:hover:not(:disabled)::-webkit-slider-thumb {
    transform: scale(1.12);
  }
  .slider::-moz-range-track {
    height: 4px;
    border-radius: var(--r-full);
    background: var(--surface-3);
  }
  .slider::-moz-range-thumb {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--surface);
    border: 2px solid var(--accent);
  }
</style>
