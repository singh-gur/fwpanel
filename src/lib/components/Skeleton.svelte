<script lang="ts">
  /* Placeholder shown until the first reading lands. The real status text is
     kept for assistive tech; only the shimmer is decorative. */
  let { lines = 2, text = "Waiting for first reading…" }: { lines?: number; text?: string } =
    $props();
</script>

<div class="skeleton" role="status">
  <span class="sr-only">{text}</span>
  {#each Array(lines) as _, i (i)}
    <span class="bar" data-i={i % 3}></span>
  {/each}
</div>

<style>
  .skeleton {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  .bar {
    height: 0.72rem;
    border-radius: var(--r-sm);
    background: linear-gradient(
      90deg,
      var(--surface-3) 0%,
      var(--surface-2) 40%,
      var(--surface-3) 80%
    );
    background-size: 240% 100%;
    animation: shimmer 1.5s var(--ease) infinite;
  }
  .bar[data-i="0"] {
    width: 100%;
  }
  .bar[data-i="1"] {
    width: 72%;
  }
  .bar[data-i="2"] {
    width: 86%;
  }
  @keyframes shimmer {
    from {
      background-position: 140% 0;
    }
    to {
      background-position: -40% 0;
    }
  }
</style>
