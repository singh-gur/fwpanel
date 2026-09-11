<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "$lib/components/Icon.svelte";
  import type { IconName } from "$lib/ui";

  /* The single card shell used by every panel. Previously each card
     re-declared the same surface/heading CSS; keeping it here means one
     place defines elevation, spacing and heading rhythm. */
  let {
    title,
    id,
    icon,
    tone = "default",
    span = false,
    actions,
    children,
  }: {
    title: string;
    /** Id for the heading, referenced by the section's aria-labelledby. */
    id: string;
    icon?: IconName;
    /** `alert` reddens the frame for a hard failure. */
    tone?: "default" | "alert";
    /** Let the card occupy the full grid width. */
    span?: boolean;
    actions?: Snippet;
    children: Snippet;
  } = $props();
</script>

<section class="card" data-tone={tone} class:span aria-labelledby={id}>
  <header class="card-head">
    {#if icon}
      <span class="card-icon"><Icon name={icon} size={15} /></span>
    {/if}
    <h2 {id}>{title}</h2>
    {#if actions}
      <div class="card-actions">{@render actions()}</div>
    {/if}
  </header>
  <div class="card-body">
    {@render children()}
  </div>
</section>

<style>
  .card {
    position: relative;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--r-lg);
    box-shadow: var(--shadow-sm);
    display: flex;
    flex-direction: column;
    min-width: 0;
    overflow: hidden;
    transition:
      box-shadow var(--dur) var(--ease),
      border-color var(--dur) var(--ease);
  }
  .card:hover {
    box-shadow: var(--shadow-md);
  }
  .card[data-tone="alert"] {
    border-color: var(--danger-line);
  }
  /* A thin top rule reads as a machined edge and separates head from chrome. */
  .card[data-tone="alert"]::before {
    content: "";
    position: absolute;
    inset: 0 0 auto 0;
    height: 2px;
    background: var(--danger);
  }
  .span {
    grid-column: 1 / -1;
  }

  .card-head {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: var(--sp-3) var(--sp-5);
    border-bottom: 1px solid var(--border);
    background: var(--surface-2);
  }
  .card-icon {
    display: flex;
    color: var(--text-faint);
  }
  h2 {
    margin: 0;
    font-size: 0.72rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
  }
  .card-actions {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: var(--sp-2);
  }

  .card-body {
    padding: var(--sp-5);
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    min-width: 0;
  }
</style>
