<script lang="ts">
  import type { DeckState, InputDeckSnapshot, PortsSnapshot } from "$lib/types";

  let {
    card,
  }: {
    card: {
      data: PortsSnapshot | InputDeckSnapshot | null;
      error: string | null;
      lastSuccessAt: Date | null;
    };
  } = $props();

  const snapshot = $derived(
    card.data && "deck_state" in card.data ? (card.data as InputDeckSnapshot) : null,
  );

  const stateLabel: Record<DeckState, string> = {
    off: "Off",
    disconnected: "Disconnected",
    turning_on: "Turning on",
    on: "On",
    force_off: "Forced off (manual override)",
    force_on: "Forced on (manual override)",
    no_detection: "On (no detection)",
  };

  const unsupported = $derived(
    card.error !== null && card.error.includes("unsupported_feature"),
  );
</script>

<section class="card" aria-labelledby="deck-heading">
  <h2 id="deck-heading">Input deck</h2>
  {#if snapshot}
    <dl class="details">
      <div><dt>Deck state</dt><dd>{stateLabel[snapshot.deck_state]}</dd></div>
      <div><dt>Touchpad</dt><dd>{snapshot.touchpad_present ? "Present" : "Not detected"}</dd></div>
    </dl>
    <p class="hint">Laptop 13 input deck power state; module inventory is not exposed.</p>
  {:else if unsupported}
    <p>This EC does not report input-deck status.</p>
  {:else if card.error}
    <p>
      <strong>Input-deck status unavailable.</strong>
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
  .error-text {
    color: #b3261e;
    font-weight: 600;
  }
  .hint {
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
