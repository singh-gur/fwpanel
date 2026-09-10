<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import type { ChargeLimitReading, ChargeLimits } from "$lib/types";

  let {
    chargeLimit,
    onApplied,
  }: {
    chargeLimit: ChargeLimitReading | null;
    /** Called after a verified apply; the parent refreshes actual status. */
    onApplied: () => void;
  } = $props();

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

  const valid = $derived(draft !== null && draft >= 25 && draft <= 100);
  const belowMinimum = $derived(
    currentMin !== null && draft !== null && draft < currentMin,
  );

  let applying = $state(false);
  let result = $state<{ ok: boolean; text: string } | null>(null);

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
      result = { ok: false, text: String(e).replace(/^service-[a-z-]+: /, "") };
    } finally {
      applying = false;
    }
  }
</script>

<section class="card" aria-labelledby="limit-heading">
  <h2 id="limit-heading">Charge limit</h2>

  {#if chargeLimit?.status === "ok"}
    <p class="charge">
      <span class="percent">{chargeLimit.limits.maximum_percent}%</span>
      <span class="meta"
        >maximum · minimum {chargeLimit.limits.minimum_percent}%</span
      >
    </p>
    <form class="apply-row" onsubmit={(e) => { e.preventDefault(); void apply(); }}>
      <label for="charge-limit-input">New maximum (%)</label>
      <input
        id="charge-limit-input"
        type="number"
        inputmode="numeric"
        min="25"
        max="100"
        step="1"
        bind:value={draft}
        disabled={applying}
      />
      <button type="submit" disabled={applying || !dirty || !valid || belowMinimum}>
        {applying ? "Applying…" : "Apply"}
      </button>
    </form>
    {#if belowMinimum}
      <p class="error-text">
        The requested maximum is below the current minimum
        ({currentMin}%); raise it above the minimum first.
      </p>
    {/if}
    {#if applying}
      <p class="hint">
        Administrator authorization may be requested; this can take up to two
        minutes. No setting changes until you approve it.
      </p>
    {/if}
    {#if result}
      <p class={result.ok ? "ok-text" : "error-text"} role="status">{result.text}</p>
    {/if}
    <p class="hint">
      Applied only after explicit Apply, verified by reading the setting back.
    </p>
  {:else if chargeLimit?.status === "failed"}
    <p>
      <strong>Charge-limit reading unavailable</strong> — {chargeLimit.message}
    </p>
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

  .apply-row {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    flex-wrap: wrap;
    margin: 0;
  }
  input {
    font: inherit;
    padding: 0.35em 0.6em;
    border-radius: 8px;
    border: 1px solid #7a7a85;
    background: #fff;
    color: inherit;
    width: 6em;
  }
  input:disabled {
    opacity: 0.6;
  }
  input:focus-visible {
    outline: 2px solid #396cd8;
    outline-offset: 2px;
  }
  .error-text {
    color: #b3261e;
    font-weight: 600;
    margin: 0;
  }
  .ok-text {
    color: #1b6e3c;
    font-weight: 600;
    margin: 0;
  }
  .hint {
    font-size: 0.85rem;
    color: #5f5f6b;
    margin: 0;
  }
  .meta {
    font-size: 0.85rem;
    color: #5f5f6b;
  }
  @media (prefers-color-scheme: dark) {
    .card {
      background: #2b2b33;
      border-color: #3f3f49;
    }
    input {
      background: #2e2e36;
      border-color: #8a8a95;
    }
    .error-text {
      color: #ff8a80;
    }
    .ok-text {
      color: #7ee2a8;
    }
  }
</style>
