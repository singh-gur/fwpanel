<script lang="ts">
  import Badge from "$lib/components/Badge.svelte";
  import Card from "$lib/components/Card.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import Skeleton from "$lib/components/Skeleton.svelte";
  import { humanError, time } from "$lib/format";
  import type { DeckState, InputDeckSnapshot, PortsSnapshot } from "$lib/types";
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

  const stateTone: Record<DeckState, Tone> = {
    off: "neutral",
    disconnected: "warn",
    turning_on: "info",
    on: "ok",
    force_off: "warn",
    force_on: "warn",
    no_detection: "ok",
  };

  const unsupported = $derived(
    card.error !== null && card.error.includes("unsupported_feature"),
  );
</script>

<Card
  title="Input deck"
  id="deck-heading"
  icon="keyboard"
  tone={card.error && !snapshot && !unsupported ? "alert" : "default"}
>
  {#if snapshot}
    <div class="rows">
      <div class="row">
        <span class="row-label">Deck state</span>
        <Badge tone={stateTone[snapshot.deck_state]} icon="chip">
          {stateLabel[snapshot.deck_state]}
        </Badge>
      </div>
      <div class="row">
        <span class="row-label">Touchpad</span>
        <Badge tone={snapshot.touchpad_present ? "ok" : "neutral"}>
          {snapshot.touchpad_present ? "Present" : "Not detected"}
        </Badge>
      </div>
    </div>
    <p class="hint">
      Laptop 13 input deck power state; module inventory is not exposed.
    </p>
  {:else if unsupported}
    <p class="notice">
      <Icon name="chip" size={15} />
      <span>This EC does not report input-deck status.</span>
    </p>
  {:else if card.error}
    <p class="notice" data-tone="danger">
      <Icon name="alert" size={15} />
      <span><strong>Input-deck status unavailable.</strong> {humanError(card.error)}</span>
    </p>
    {#if card.lastSuccessAt}
      <p class="hint">Last successful read: {time(card.lastSuccessAt)}.</p>
    {/if}
  {:else}
    <Skeleton lines={2} />
  {/if}
</Card>

<style>
  .rows {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-3);
    flex-wrap: wrap;
    padding: var(--sp-2) 0;
    border-bottom: 1px solid var(--border);
  }
  .row:last-child {
    border-bottom: none;
    padding-bottom: 0;
  }
  .row-label {
    font-size: 0.72rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-faint);
  }
</style>
