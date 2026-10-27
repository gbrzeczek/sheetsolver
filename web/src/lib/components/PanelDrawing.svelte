<!-- One panel of a plan, drawn to scale (DESIGN.md §4.6). -->
<script lang="ts">
  import type { Pattern, PlanNode } from "../api/types";
  import type { Cut } from "../plan/cutorder";
  import { layout, BADGE_RADIUS, type LabelMode, type Shape } from "../plan/drawing";

  interface Props {
    pattern: Pattern;
    kerf: number;
    grain(partId: string): boolean;
    /** Width of the panel in px, at most; on a narrower screen it is laid out at the width it is shown at. */
    width: number;
    labels?: LabelMode;
    rulers?: boolean;
    cuts?: readonly Cut[];
    /** The current cut: its saw line and result are highlighted, earlier badges faded. */
    current?: number | null;
    nameSize?: number;
    title: string;
    oncut?(number: number): void;
    onpart?(node: PlanNode): void;
    selected?: PlanNode | null;
  }
  let {
    pattern, kerf, grain, width, labels = "full", rulers = false, cuts, current = null,
    nameSize = 12, title, oncut, onpart, selected = null,
  }: Props = $props();

  const uid = $props.id();
  // Laid out at the width it is shown at, so labels keep their size on a
  // phone instead of shrinking with the drawing (DESIGN.md §4.6).
  let shown = $state(0);
  const fitted = $derived(shown > 0 ? Math.min(width, Math.round(shown)) : width);
  const d = $derived(layout(pattern, { width: fitted, labels, rulers, cuts, kerf, grain, nameSize }));
  const currentBadge = $derived(d.badges.find((b) => b.number === current));

  function center(s: Shape) {
    return { cx: s.x + s.w / 2, cy: s.y + s.h / 2 };
  }
  const r = (n: number) => Math.round(n * 100) / 100;
  const textW = (t: string, size: number) => t.length * size * 0.56;
</script>

{#snippet label(s: Shape, cls: string, nameSz: number, detailSz: number)}
  {@const { cx, cy } = center(s)}
  {@const rot = s.fit.mode !== "none" && s.fit.rotate ? `rotate(-90 ${r(cx)} ${r(cy)})` : undefined}
  {#if s.fit.mode === "stacked"}
    <g class={cls} transform={rot}>
      <text x={r(cx)} y={r(cy - 1.5)} text-anchor="middle" class="p-name" font-size={nameSz}>{s.name}</text>
      <text x={r(cx)} y={r(cy + detailSz + 1)} text-anchor="middle" class="p-dim" font-size={detailSz}>{s.detail}</text>
    </g>
  {:else if s.fit.mode === "single"}
    {@const sz = s.detail ? detailSz : nameSz}
    <text x={r(cx)} y={r(cy + sz * 0.35)} text-anchor="middle" class="{cls} {s.detail ? 'p-dim' : 'p-name'}" font-size={sz} transform={rot}>{s.detail ?? s.name}</text>
  {/if}
{/snippet}

<svg viewBox="0 0 {r(d.width)} {r(d.height)}" class="plan-svg" role="img" aria-labelledby="{uid}-t" bind:clientWidth={shown}>
  <title id="{uid}-t">{title}</title>
  <defs>
    <pattern id="{uid}-hatch" width="6" height="6" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
      <rect width="6" height="6" class="p-offcut-bg" /><line x1="0" y1="0" x2="0" y2="6" class="p-hatch" />
    </pattern>
  </defs>
  <!-- The panel shows through wherever the saw removes material. -->
  <rect x={d.panel.x} y={d.panel.y} width={r(d.panel.w)} height={r(d.panel.h)} class="p-kerf" />

  {#each d.shapes as s, i (i)}
    {#if s.kind === "part"}
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
      <g class:piece-hit={!!onpart} onclick={() => onpart?.(s.node)}>
        <rect x={r(s.x)} y={r(s.y)} width={r(s.w)} height={r(s.h)} class="p-part" class:p-selected={selected === s.node} />
        {#if labels !== "none" && s.grain && s.w > 70 && s.h > 48}
          <g transform="translate({r(s.x + s.w - 19)} {r(s.y + s.h - 12)})" class="p-glyph">
            <path d="M1 4h12M3.5 1.5 1 4l2.5 2.5M10.5 1.5 13 4l-2.5 2.5" fill="none" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" />
          </g>
        {/if}
        {#if labels !== "none" && s.rotated && s.w > 30 && s.h > 24}
          <g transform="translate({r(s.x + s.w - 16)} {r(s.y + s.h - 16)})" class="p-glyph">
            <path d="M9.6 4.3A4 4 0 1 0 10 6" fill="none" stroke-width="1.3" stroke-linecap="round" />
            <path d="M10.4 1.8v2.8H7.6" fill="none" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" />
          </g>
        {/if}
        {@render label(s, "p-part-text", d.nameSize, d.detailSize)}
      </g>
    {:else if s.kind === "offcut"}
      <rect x={r(s.x)} y={r(s.y)} width={r(s.w)} height={r(s.h)} fill="url(#{uid}-hatch)" class="p-offcut" />
      {@render label(s, "p-offcut-text", d.nameSize, d.detailSize - 0.5)}
    {:else}
      <rect x={r(s.x + 0.75)} y={r(s.y + 0.75)} width={r(s.w - 1.5)} height={r(s.h - 1.5)} class="p-leftover" />
      {@render label(s, "p-leftover-text", d.nameSize, d.detailSize)}
    {/if}
  {/each}

  <rect x={d.panel.x} y={d.panel.y} width={r(d.panel.w)} height={r(d.panel.h)} class="p-edge" />

  {#if d.rulers}
    {@const R = d.rulers}
    {@const y0 = d.panel.y - 16}
    {@const x1 = d.panel.x + d.panel.w + 16}
    <g class="p-ruler">
      <line x1={R.top.from} y1={y0} x2={r(R.top.to)} y2={y0} />
      <line x1={R.top.from} y1={y0 - 5} x2={R.top.from} y2={y0 + 5} />
      <line x1={r(R.top.to)} y1={y0 - 5} x2={r(R.top.to)} y2={y0 + 5} />
      <rect x={r((R.top.from + R.top.to) / 2 - textW(R.top.label ?? "", 11) / 2 - 6)} y={y0 - 8} width={r(textW(R.top.label ?? "", 11) + 12)} height="16" class="p-ruler-bg" />
      <text x={r((R.top.from + R.top.to) / 2)} y={y0 + 4} text-anchor="middle">{R.top.label}</text>

      <line x1={r(x1)} y1={R.right.from} x2={r(x1)} y2={r(R.right.to)} />
      <line x1={r(x1 - 5)} y1={R.right.from} x2={r(x1 + 5)} y2={R.right.from} />
      <line x1={r(x1 - 5)} y1={r(R.right.to)} x2={r(x1 + 5)} y2={r(R.right.to)} />
      <g transform="rotate(-90 {r(x1)} {r((R.right.from + R.right.to) / 2)})">
        <rect x={r(x1 - textW(R.right.label ?? "", 11) / 2 - 6)} y={r((R.right.from + R.right.to) / 2 - 8)} width={r(textW(R.right.label ?? "", 11) + 12)} height="16" class="p-ruler-bg" />
        <text x={r(x1)} y={r((R.right.from + R.right.to) / 2 + 4)} text-anchor="middle">{R.right.label}</text>
      </g>

      {#each R.strips as m, i (i)}
        {#if R.stripsSide === "left"}
          {@const xr = d.panel.x - 12}
          <line x1={xr - 4} y1={r(m.from)} x2={xr + 4} y2={r(m.from)} />
          <line x1={xr - 4} y1={r(m.to)} x2={xr + 4} y2={r(m.to)} />
          <line x1={xr} y1={r(m.from)} x2={xr} y2={r(m.to)} />
          {#if m.label}<text x={xr - 8} y={r((m.from + m.to) / 2 + 4)} text-anchor="end" class:p-ruler-weak={m.weak}>{m.label}</text>{/if}
        {:else}
          {@const yr = d.panel.y + d.panel.h + 12}
          <line x1={r(m.from)} y1={yr - 4} x2={r(m.from)} y2={yr + 4} />
          <line x1={r(m.to)} y1={yr - 4} x2={r(m.to)} y2={yr + 4} />
          <line x1={r(m.from)} y1={yr} x2={r(m.to)} y2={yr} />
          {#if m.label}<text x={r((m.from + m.to) / 2)} y={yr + 14} text-anchor="middle" class:p-ruler-weak={m.weak}>{m.label}</text>{/if}
        {/if}
      {/each}
    </g>
  {/if}

  {#if currentBadge}
    {@const l = currentBadge.line}
    {@const res = currentBadge.result}
    <rect x={r(res.x)} y={r(res.y)} width={r(res.w)} height={r(res.h)} class="p-result" />
    <rect x={r(l.x - (l.w < l.h ? 0.75 : 0))} y={r(l.y - (l.w < l.h ? 0 : 0.75))} width={r(l.w + (l.w < l.h ? 1.5 : 0))} height={r(l.h + (l.w < l.h ? 0 : 1.5))} class="p-cutline" />
  {/if}
  {#each d.badges as b (b.number)}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <g
      class="p-badge"
      class:on={b.number === current}
      class:done={current !== null && b.number < current}
      onclick={() => oncut?.(b.number)}
      style:cursor={oncut ? "pointer" : undefined}
    >
      <circle cx={r(b.cx)} cy={r(b.cy)} r={BADGE_RADIUS} />
      <text x={r(b.cx)} y={r(b.cy + 3.6)} text-anchor="middle">{b.number}</text>
    </g>
  {/each}
</svg>
