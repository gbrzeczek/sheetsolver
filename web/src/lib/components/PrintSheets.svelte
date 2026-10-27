<!-- The job sheet for paper: one page per panel, with its drawing, cut
     order and parts (owner's decision; DESIGN.md §9). Hidden on screen. -->
<script lang="ts">
  import PanelDrawing from "./PanelDrawing.svelte";
  import CutList from "./CutList.svelte";
  import type { Instance, JobStatus, PlanReport } from "../api/types";
  import { cutOrder } from "../plan/cutorder";
  import { mm, panelTrims, patternStats, percent, size } from "../plan/tree";
  import { headline, panelsLine } from "../plan/summary";
  import { optionTitle } from "../api/fields";

  interface Props {
    job: JobStatus;
    instance: Instance;
    best: PlanReport;
    grain(partId: string): boolean;
  }
  let { job, instance, best, grain }: Props = $props();

  const patterns = $derived(best.plan.patterns);
  // A generated panel name ("Panel 1") says nothing on paper.
  const named = (id: string) => (/^Panel \d+$/.test(id) ? "" : `${id} · `);
  const kerf = $derived(instance.saw.kerf);
  const settings = $derived(
    [
      `Kerf ${mm(kerf)} mm`,
      instance.saw.stages === "unlimited" ? "unlimited stages" : `${instance.saw.stages} stages`,
      `first cuts: ${optionTitle("Saw.first_cuts", instance.saw.first_cuts).toLowerCase()}`,
      `last-stage trimming: ${optionTitle("Saw.cut_type", instance.saw.cut_type).toLowerCase()}`,
      instance.saw.minimum_offcut_width ? `offcuts at least ${mm(instance.saw.minimum_offcut_width)} mm` : null,
    ]
      .filter(Boolean)
      .join(" · "),
  );
</script>

<div class="print-only" aria-hidden="true">
  {#each patterns as pattern, i (i)}
    {@const order = cutOrder(pattern, kerf, instance.saw.stages)}
    {@const s = patternStats(pattern, panelTrims(instance, pattern))}
    {@const counts = s.partLeaves.reduce((m, l) => (l.kind === "part" ? m.set(l.part_id, (m.get(l.part_id) ?? 0) + 1) : m), new Map<string, number>())}
    <section class="print-sheet">
      <header class="print-head">
        <div>
          <div class="eyebrow">{job.name || "Cutting plan"} · {headline(instance, best.metrics)} · {panelsLine(instance, best.plan)}</div>
          <h2>Panel {i + 1} of {patterns.length}{#if pattern.repeat > 1}{" "}<small>(cut {pattern.repeat} panels this way)</small>{/if}</h2>
          <small>{named(pattern.panel_id)}{size(pattern.root)} mm · {s.parts} parts · {percent(s.wasteFraction)} waste · {settings}</small>
        </div>
      </header>
      <PanelDrawing {pattern} trims={panelTrims(instance, pattern)} {kerf} {grain} width={900} labels="full" rulers cuts={order.cuts} current={null} nameSize={11}
        title="Panel {i + 1}" />
      <div class="print-cols">
        <div class="print-cuts">
          <h3>Cut order <small>· {order.cuts.length} cuts; each size is the piece to cut off</small></h3>
          <CutList {order} current={0} onselect={() => {}} />
        </div>
        <div>
          <h3>On this panel</h3>
          {#each [...counts] as [pid, n] (pid)}
            {@const part = instance.parts.find((p) => p.id === pid)}
            <div class="manifest-row"><div>☐ {pid} <span>{part ? size(part) : ""}</span></div><strong>× {n}</strong></div>
          {/each}
        </div>
      </div>
    </section>
  {/each}
</div>
