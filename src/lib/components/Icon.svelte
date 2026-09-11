<script lang="ts">
  /* Inline stroke icons. Kept local (no icon dependency) and drawn on a
     24x24 grid so every glyph shares weight and optical size. */
  import type { IconName } from "$lib/ui";

  let {
    name,
    size = 16,
  }: {
    name: IconName;
    size?: number;
  } = $props();
</script>

<svg
  class="icon"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.75"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
  focusable="false"
>
  {#if name === "battery"}
    <rect x="2" y="7" width="16" height="10" rx="2.5" />
    <path d="M21 10.5v3" />
  {:else if name === "bolt"}
    <path d="M13 2 4.5 13.5H11L10 22l8.5-11.5H12z" />
  {:else if name === "plug"}
    <path d="M9 2v6M15 2v6" />
    <path d="M6 8h12v3a6 6 0 0 1-6 6 6 6 0 0 1-6-6z" />
    <path d="M12 17v5" />
  {:else if name === "gauge"}
    <path d="M4 18a8 8 0 1 1 16 0" />
    <path d="m12 14 4-4" />
  {:else if name === "keyboard"}
    <rect x="2" y="5" width="20" height="14" rx="2.5" />
    <path d="M6 9h.01M10 9h.01M14 9h.01M18 9h.01M6 13h.01M18 13h.01M9 13h6" />
  {:else if name === "refresh"}
    <path d="M21 12a9 9 0 1 1-2.64-6.36" />
    <path d="M21 3v5h-5" />
  {:else if name === "check"}
    <path d="m4 12.5 5 5L20 6.5" />
  {:else if name === "alert"}
    <path d="M12 3.5 2.5 20h19z" />
    <path d="M12 10v4M12 17.2h.01" />
  {:else if name === "shield"}
    <path d="M12 2.5 20 6v6c0 4.5-3.2 8.2-8 9.5-4.8-1.3-8-5-8-9.5V6z" />
    <path d="m9 12 2 2 4-4" />
  {:else if name === "chip"}
    <rect x="6" y="6" width="12" height="12" rx="2" />
    <path d="M10 2v4M14 2v4M10 18v4M14 18v4M2 10h4M2 14h4M18 10h4M18 14h4" />
  {:else if name === "arrow-down"}
    <path d="M12 4v16M6 14l6 6 6-6" />
  {:else if name === "arrow-up"}
    <path d="M12 20V4M6 10l6-6 6 6" />
  {/if}
</svg>

<style>
  .icon {
    display: block;
    flex: none;
  }
</style>
