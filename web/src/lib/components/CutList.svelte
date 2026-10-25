<!-- The cut order of one panel (DESIGN.md §2, "Cutting plan, done"). -->
<script lang="ts">
  import type { PlanNode } from "../api/types";
  import type { Cut, CutOrder } from "../plan/cutorder";
  import { mm, size } from "../plan/tree";

  interface Props {
    order: CutOrder;
    current: number;
    onselect(number: number): void;
  }
  let { order, current, onselect }: Props = $props();

  type Group = { title: string; meta: string; cuts: Cut[] };
  const groups = $derived.by(() => {
    const out: Group[] = [];
    for (const cut of order.cuts) {
      let g = out[out.length - 1];
      if (!g || (g.title !== cut.group && !(g.title === "Panel into strips" && cut.group === "panel"))) {
        const strip = [...order.names.entries()].find(([, n]) => n === cut.group)?.[0];
        g = cut.group === "panel"
          ? { title: "Panel into strips", meta: cut.direction === "along" ? "along the length" : "across the length", cuts: [] }
          : { title: cut.group, meta: strip ? size(strip) : "", cuts: [] };
        out.push(g);
      }
      g.cuts.push(cut);
    }
    return out;
  });

  function what(node: PlanNode): { bold: string | null; rest: string } {
    if (node.kind === "part") return { bold: node.part_id, rest: size(node) };
    if (node.kind === "leftover") return { bold: "leftover", rest: size(node) };
    if (node.kind === "offcut") return { bold: null, rest: `offcut ${size(node)}` };
    return { bold: order.names.get(node) ?? "piece", rest: size(node) };
  }

  let list: HTMLOListElement | undefined = $state();
  // Keep the current cut in view by scrolling the list only, never the page.
  $effect(() => {
    const row = list?.querySelector<HTMLElement>(`[data-cut="${current}"]`);
    const box = list?.closest<HTMLElement>(".scroll");
    if (!row || !box || box.scrollHeight <= box.clientHeight) return;
    const top = row.offsetTop - box.offsetTop;
    if (top < box.scrollTop) box.scrollTop = top - 40;
    else if (top + row.offsetHeight > box.scrollTop + box.clientHeight) box.scrollTop = top + row.offsetHeight - box.clientHeight + 16;
  });
</script>

<ol class="cutlist" bind:this={list}>
  {#each groups as g, gi (gi)}
    <li class="group"><span>{g.title}</span><span class="num">{g.meta}</span></li>
    {#each g.cuts as cut (cut.number)}
      {@const p = what(cut.produces)}
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
      <li
        class="cut"
        class:on={cut.number === current}
        class:done={cut.number < current}
        aria-current={cut.number === current ? "step" : undefined}
        data-cut={cut.number}
        onclick={() => onselect(cut.number)}
      >
        <span class="n">{cut.number}</span>
        <div>
          <div class="m">{mm(cut.measure)} mm<em>{cut.trim ? "trim, " : ""}{cut.direction === "along" ? "along the length" : "across the length"}</em></div>
          <div class="r">→ {#if p.bold}<b>{p.bold}</b>{" "}{/if}{p.rest}</div>
          {#if cut.leaves}
            {@const l = what(cut.leaves)}
            <div class="r">
              leaves {#if cut.leavesSawDust}saw dust ({mm(Math.min(cut.leaves.length, cut.leaves.width))} mm){:else}{#if l.bold}<b>{l.bold}</b>{" "}{/if}{l.rest}{/if}
            </div>
          {/if}
        </div>
      </li>
    {/each}
  {/each}
</ol>
