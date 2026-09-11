<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "$lib/components/Icon.svelte";
  import type { IconName, Tone } from "$lib/ui";

  /* Semantic pill. Tone carries meaning through colour AND the label text,
     never colour alone. */

  let {
    tone = "neutral",
    icon,
    strong = false,
    children,
  }: {
    tone?: Tone;
    icon?: IconName;
    strong?: boolean;
    children: Snippet;
  } = $props();
</script>

<span class="badge" data-tone={tone} class:strong>
  {#if icon}<Icon name={icon} size={12} />{/if}
  <span>{@render children()}</span>
</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    gap: 0.3em;
    padding: 0.16em 0.62em;
    border-radius: var(--r-full);
    border: 1px solid var(--tone-line);
    background: var(--tone-soft);
    color: var(--tone-fg);
    font-size: 0.78rem;
    font-weight: 600;
    letter-spacing: 0.01em;
    line-height: 1.5;
    white-space: nowrap;
  }
  .badge[data-tone="neutral"] {
    --tone-fg: var(--text-muted);
    --tone-soft: var(--idle-soft);
    --tone-line: var(--idle-line);
  }
  .badge[data-tone="accent"] {
    --tone-fg: var(--accent);
    --tone-soft: var(--accent-soft);
    --tone-line: var(--accent-line);
  }
  .badge[data-tone="ok"] {
    --tone-fg: var(--ok);
    --tone-soft: var(--ok-soft);
    --tone-line: var(--ok-line);
  }
  .badge[data-tone="warn"] {
    --tone-fg: var(--warn);
    --tone-soft: var(--warn-soft);
    --tone-line: var(--warn-line);
  }
  .badge[data-tone="danger"] {
    --tone-fg: var(--danger);
    --tone-soft: var(--danger-soft);
    --tone-line: var(--danger-line);
  }
  .badge[data-tone="info"] {
    --tone-fg: var(--info);
    --tone-soft: var(--info-soft);
    --tone-line: var(--info-line);
  }
  .strong {
    border-width: 1.5px;
    font-weight: 700;
  }
</style>
