<!-- Panel navigation: "All panels", then one card per pattern. -->
<script lang="ts">
  import PanelDrawing from "./PanelDrawing.svelte";
  import type { Instance, Pattern } from "../api/types";
  import { panelTrims, patternStats, percent } from "../plan/tree";
  import { href } from "../router.svelte";

  interface Props {
    id: string;
    patterns: readonly Pattern[];
    /** null: "All panels"; else the 1-based panel. */
    current: number | null;
    kerf: number;
    grain(partId: string): boolean;
    /** The job, for each panel's trims. */
    instance?: Instance;
  }
  let { id, patterns, current, kerf, grain, instance }: Props = $props();
</script>

<nav class="sheet-nav" aria-label="Panels">
  <a class="sheet-choice" href={href({ name: "plan", id, panel: null, cut: null })} aria-current={current === null ? "page" : undefined}>
    <div class="all" aria-hidden="true"><span></span><span></span><span></span></div>
    <strong>All panels</strong><small>Overview and parts</small>
  </a>
  {#each patterns as pattern, i (i)}
    {@const s = patternStats(pattern, panelTrims(instance, pattern))}
    <a class="sheet-choice" href={href({ name: "plan", id, panel: i + 1, cut: null })} aria-current={current === i + 1 ? "page" : undefined}>
      <PanelDrawing {pattern} trims={panelTrims(instance, pattern)} {kerf} {grain} width={144} labels="none" title="Panel {i + 1}" />
      <strong>Panel {i + 1}{#if pattern.repeat > 1}{" "}<small style="display:inline">× {pattern.repeat}</small>{/if}</strong>
      <small>{s.parts} parts · {percent(s.wasteFraction)} waste</small>
      {#if s.leftovers.length}<small>1 strip to keep</small>{/if}
    </a>
  {/each}
</nav>
