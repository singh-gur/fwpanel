<script lang="ts">
  import type { Tone } from "$lib/ui";

  /* Radial charge gauge. Every data-driven value is an SVG presentation
     attribute (stroke-dasharray, cx/cy) rather than an inline style, because
     the production CSP is `style-src 'self'` and blocks style attributes. */
  let {
    value,
    tone = "ok",
    label,
    sublabel,
    limit = null,
    dim = false,
    size = 168,
  }: {
    /** 0–100. */
    value: number;
    tone?: Tone;
    /** Big text in the middle, e.g. "82%". */
    label: string;
    sublabel?: string;
    /** Optional charge limit; the capped-off arc is shaded on the track. */
    limit?: number | null;
    /** Fade the dial when the reading is stale. */
    dim?: boolean;
    size?: number;
  } = $props();

  const R = 52;
  const CIRC = 2 * Math.PI * R;
  const clamped = $derived(Math.max(0, Math.min(100, value)));
  const dash = $derived(`${(clamped / 100) * CIRC} ${CIRC}`);

  // The arc from the configured maximum round to 100%: capacity the charge
  // limit puts out of reach. Drawn muted, under the fill.
  const capped = $derived.by(() => {
    if (limit === null) return null;
    const at = Math.max(0, Math.min(100, limit));
    if (at >= 100) return null;
    return {
      dash: `${((100 - at) / 100) * CIRC} ${CIRC}`,
      rotate: `rotate(${(at / 100) * 360 - 90} 60 60)`,
    };
  });
</script>

<div class="gauge" data-tone={tone} class:dim>
  <svg width={size} height={size} viewBox="0 0 120 120" role="img" aria-label={label}>
    <circle class="track" cx="60" cy="60" r={R} stroke-width="9" fill="none" />
    {#if capped}
      <circle
        class="capped"
        cx="60"
        cy="60"
        r={R}
        stroke-width="9"
        fill="none"
        stroke-dasharray={capped.dash}
        transform={capped.rotate}
      />
    {/if}
    <circle
      class="fill"
      cx="60"
      cy="60"
      r={R}
      stroke-width="9"
      fill="none"
      stroke-linecap="round"
      stroke-dasharray={dash}
      transform="rotate(-90 60 60)"
    />
  </svg>
  <div class="readout">
    <span class="value">{label}</span>
    {#if sublabel}<span class="sub">{sublabel}</span>{/if}
  </div>
</div>

<style>
  .gauge {
    position: relative;
    display: grid;
    place-items: center;
    flex: none;
  }
  .gauge svg {
    display: block;
  }
  .gauge[data-tone="ok"] {
    --dial: var(--ok);
  }
  .gauge[data-tone="warn"] {
    --dial: var(--warn);
  }
  .gauge[data-tone="danger"] {
    --dial: var(--danger);
  }
  .gauge[data-tone="accent"] {
    --dial: var(--accent);
  }
  .gauge[data-tone="info"] {
    --dial: var(--info);
  }
  .gauge[data-tone="neutral"] {
    --dial: var(--idle);
  }
  .dim {
    opacity: 0.55;
  }

  .track {
    stroke: var(--surface-3);
  }
  .fill {
    stroke: var(--dial);
    transition: stroke-dasharray 600ms var(--ease);
  }
  .capped {
    stroke: var(--text-faint);
    stroke-opacity: 0.28;
  }

  .readout {
    position: absolute;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1px;
  }
  .value {
    font-size: 2.1rem;
    font-weight: 650;
    letter-spacing: -0.02em;
    line-height: 1;
    font-variant-numeric: tabular-nums;
  }
  .sub {
    font-size: 0.75rem;
    color: var(--text-muted);
    text-align: center;
  }
</style>
