<!-- The Cutting plan tab: progress while searching, then all panels or one
     panel with its cut order (DESIGN.md §2; mockups/solving.html,
     plan-overview.html, plan-panel.html, states.html). In the workbench
     (`pane`) it is the right column: all panels, without the panel list. -->
<script lang="ts">
  import PanelDrawing from "../lib/components/PanelDrawing.svelte";
  import CutList from "../lib/components/CutList.svelte";
  import SheetNav from "../lib/components/SheetNav.svelte";
  import Legend from "../lib/components/Legend.svelte";
  import PrintSheets from "../lib/components/PrintSheets.svelte";
  import { session } from "../lib/job/session.svelte";
  import { router, href } from "../lib/router.svelte";
  import { cutOrder } from "../lib/plan/cutorder";
  import { area, mm, patternStats, percent, size, squareMetres, leaves } from "../lib/plan/tree";
  import { checklist, headline, oversizedParts, panelsLine, totalParts } from "../lib/plan/summary";
  import { isFinal, type PlanNode } from "../lib/api/types";

  interface Props {
    id: string;
    panel: number | null;
    cut: number | null;
    /** In the workbench, next to the job: no panel list, no *Edit job*. */
    pane?: boolean;
  }
  let { id, panel, cut, pane = false }: Props = $props();

  $effect(() => {
    void session.open(id);
  });

  const job = $derived(session.jobId === id ? session.job : null);
  const best = $derived(session.jobId === id ? session.best : null);
  const instance = $derived(job?.instance);
  const kerf = $derived(instance?.saw.kerf ?? 0);
  const grainOf = $derived.by(() => {
    const grains = new Map(instance?.parts.map((p) => [p.id, p.grain ?? false]));
    return (partId: string) => grains.get(partId) ?? false;
  });
  const searching = $derived(job !== null && !isFinal(job.state));
  const name = $derived(session.jobName);
  const stale = $derived(job !== null && session.stale);
  const headingTag = $derived(pane ? "h2" : "h1");

  // The time bar moves between events with the local clock.
  let now = $state(performance.now());
  $effect(() => {
    if (!searching) return;
    const timer = setInterval(() => (now = performance.now()), 100);
    return () => clearInterval(timer);
  });
  const elapsed = $derived(
    job ? Math.min(job.time_limit, job.elapsed + (job.state === "running" ? (now - session.elapsedAt) / 1000 : 0)) : 0,
  );

  const patterns = $derived(best?.plan.patterns ?? []);
  const panelIndex = $derived(panel !== null && panel >= 1 && panel <= patterns.length ? panel - 1 : null);
  const pattern = $derived(panelIndex !== null ? patterns[panelIndex]! : null);
  const order = $derived(pattern && instance ? cutOrder(pattern, kerf, instance.saw.stages) : null);
  const current = $derived(order ? Math.min(Math.max(cut ?? 1, 1), Math.max(order.cuts.length, 1)) : 1);
  const currentCut = $derived(order?.cuts[current - 1] ?? null);

  // A link to a panel the plan does not have: show all panels, and say why.
  $effect(() => {
    if (best && panel !== null && panelIndex === null) {
      session.missingPanel = { id, panel };
      router.go({ name: "plan", id, panel: null, cut: null }, true);
    } else if (panel !== null) {
      session.missingPanel = null;
    }
  });
  const missing = $derived(session.missingPanel?.id === id ? session.missingPanel.panel : null);

  let selected = $state<PlanNode | null>(null);
  // A selection belongs to the panel it was made on: another panel, or a
  // better plan replacing this one, starts without one.
  $effect.pre(() => {
    void pattern;
    selected = null;
  });

  function goCut(n: number) {
    selected = null;
    router.go({ name: "plan", id, panel, cut: n }, true);
  }

  function keys(event: KeyboardEvent) {
    if (!order || (event.target as HTMLElement).closest("input, select, textarea")) return;
    if (event.key === "ArrowDown" || event.key === "ArrowRight") goCut(Math.min(current + 1, order.cuts.length));
    else if (event.key === "ArrowUp" || event.key === "ArrowLeft") goCut(Math.max(current - 1, 1));
    else return;
    event.preventDefault();
  }

  const longerSearch = $derived(job ? Math.min(600, Math.max(60, job.time_limit * 6)) : 60);

  function what(node: PlanNode): string {
    if (node.kind === "part") return `${node.part_id} ${size(node)}`;
    if (node.kind === "leftover") return `leftover ${size(node)}`;
    if (node.kind === "offcut") return `offcut ${size(node)}`;
    return `${order?.names.get(node) ?? "piece"} ${size(node)}`;
  }

  function leftoverOf() {
    for (const [i, p] of patterns.entries()) {
      const leftover = [...leaves(p.root)].find((l) => l.kind === "leftover");
      if (leftover) return { leftover, panel: i + 1 };
    }
    return null;
  }
  const keep = $derived(leftoverOf());

  // The overview's parts checklist: parts not cut first.
  const checklistRows = $derived(
    instance && best ? checklist(instance, best.plan).sort((a, b) => Number(a.cut >= a.needed) - Number(b.cut >= b.needed)) : [],
  );
  // The panel view's lists: parts on this panel, and offcuts worth keeping (largest first).
  const onPanel = $derived.by(() => {
    const counts = new Map<string, number>();
    if (pattern) for (const l of patternStats(pattern).partLeaves) if (l.kind === "part") counts.set(l.part_id, (counts.get(l.part_id) ?? 0) + 1);
    return [...counts];
  });
  const offs = $derived(
    pattern ? patternStats(pattern).offcuts.filter((o) => Math.min(o.length, o.width) >= kerf).sort((a, b) => area(b) - area(a)) : [],
  );
</script>

<svelte:window onkeydown={keys} />

{#snippet staleBar()}
  {#if stale && job}
    <div class="stale-bar" role="status">
      <div>
        <strong>The job has changed since this {best ? (searching ? "search started" : "plan was found") : "search"}.</strong>
        {best ? "The plan below is for the job as it was." : ""}
      </div>
      <div class="actions">
        <button class="button primary" type="button" disabled={session.submitting} onclick={() => void session.submit()}>
          {session.submitting ? "Sending…" : "Find plan again"}
        </button>
        <button class="link" type="button" onclick={() => session.edit()}>Edit this plan's job</button>
      </div>
    </div>
  {/if}
{/snippet}

<!-- An action with `editor` goes to the job, which the workbench already shows. -->
{#snippet stateBlock(tag: string, title: string, text: string, actions: { label: string; primary?: boolean; editor?: boolean; run(): void }[], items: string[] = [])}
  {@const shown = actions.filter((a) => !(pane && a.editor))}
  <main class={pane ? "pane-main" : "narrow"} class:is-stale={stale}>
    {@render staleBar()}
    <section class="state" aria-live="polite">
      <div class="eyebrow">{tag}</div>
      <h2>{title}</h2>
      <p>{text}</p>
      {#if items.length}<ul>{#each items as item, i (i)}<li>{item}</li>{/each}</ul>{/if}
      {#if shown.length}
        <div class="actions">
          {#each shown as a (a.label)}<button class="button" class:primary={a.primary} type="button" onclick={a.run}>{a.label}</button>{/each}
        </div>
      {/if}
    </section>
  </main>
{/snippet}

{#snippet missingNote()}
  {#if missing !== null}
    <p class="statusline" role="status" style="margin:0 0 12px">
      Panel {missing} is not in this plan: it uses {patterns.length} {patterns.length === 1 ? "panel" : "panels"}{searching ? " since a better plan was found" : ""}.
    </p>
  {/if}
{/snippet}

{#snippet figures()}
  {#if best && instance}
    {@const m = best.metrics}
    {@const total = totalParts(instance)}
    <div class="stats">
      <div class="stat"><strong>{m.panels_used}</strong>{m.panels_used === 1 ? "panel" : "panels"}
        <small>{patterns.length === 1 || new Set(patterns.map((p) => p.panel_id)).size === 1 ? `${size(patterns[0]!.root)} mm each` : "several sizes"}</small></div>
      <div class="stat"><strong>{m.parts_cut} / {total}</strong>parts
        {#if m.parts_cut >= total}<small class="ok">✓ No missing parts</small>{:else}<small style="color:var(--bad)">{total - m.parts_cut} not cut</small>{/if}</div>
      <div class="stat"><strong>{percent(m.waste_fraction)}</strong>waste<small>{squareMetres(m.waste_area)} offcuts and saw dust</small></div>
      {#if keep}
        <div class="stat"><strong>1</strong>leftover to keep<small>{size(keep.leftover)} mm · panel {keep.panel}</small></div>
      {:else if instance.objective === "open_dimension_x"}
        <div class="stat"><strong>{mm(m.used_length)}</strong>mm used<small>of {mm(instance.panels[0]?.length ?? 0)} mm length</small></div>
      {:else if instance.objective === "open_dimension_y"}
        <div class="stat"><strong>{mm(m.used_width)}</strong>mm used<small>of {mm(instance.panels[0]?.width ?? 0)} mm width</small></div>
      {/if}
    </div>
  {/if}
{/snippet}

{#snippet details()}
  {#if job && instance && best}
    <details class="more">
      <summary>Cutting settings and search details</summary>
      <p>
        Kerf {mm(kerf)} mm · {instance.saw.stages === "unlimited" ? "unlimited" : instance.saw.stages} cutting stages ·
        first cuts {instance.saw.first_cuts === "either" ? "either way" : instance.saw.first_cuts.replace("_", " the ")} ·
        last-stage trimming {instance.saw.cut_type === "non_exact" ? "allowed" : instance.saw.cut_type === "exact" ? "not allowed" : "one part per strip"}.
      </p>
      <p>
        This is plan {best.number}, found after {best.found_after.toFixed(1)} s of a {job.time_limit} s search.
        {job.ended_by === "complete"
          ? "The search ran to its end: a longer time limit would give the same plan."
          : "Reaching the time limit does not prove that fewer panels or less waste is impossible."}
      </p>
    </details>
  {/if}
{/snippet}

{#if session.trouble?.kind === "not_found" && session.jobId === id}
  {@render stateBlock("Not found", "This plan is no longer on the server",
    "Plans are kept for an hour after the search ends. Your job is still in this browser, so you can search again.",
    [{ label: "Go to job setup", primary: true, editor: true, run: () => router.go({ name: "setup" }) }])}
{:else if session.trouble?.kind === "unreachable" && session.jobId === id}
  {@render stateBlock("Connection", "The server cannot be reached", session.trouble.message,
    [{ label: "Try again", primary: true, run: () => { session.jobId = null; void session.open(id); } }])}
{:else if !job || !instance}
  <main class={pane ? "pane-main" : "narrow"}><p class="muted" aria-live="polite">Loading the cutting plan…</p></main>
{:else if !best && job.state === "failed"}
  {@render stateBlock("Failed", "The search stopped with an error",
    `This is a fault in Sheetsolver, not in your job. Details for a bug report: ${job.message ?? "none"}`,
    [{ label: "Search again", primary: true, run: () => void session.again() }, { label: "Edit job", editor: true, run: () => session.edit() }])}
{:else if !best && job.state === "cancelled"}
  {@render stateBlock("Cancelled", "Stopped before the first plan",
    "The search was stopped before it found a plan. The first plan usually appears within a second.",
    [{ label: "Search again", primary: true, run: () => void session.again() }, { label: "Edit job", editor: true, run: () => session.edit() }])}
{:else if !best && job.state === "done"}
  {@const big = oversizedParts(instance)}
  {#if instance.objective === "knapsack"}
    {@render stateBlock("Done · no plan", "No part fits on any panel",
      "With the goal “Best use of the panels I have”, the search cuts as many parts as fit, but none fits on the panels listed.",
      [{ label: "Edit job", primary: true, editor: true, run: () => session.edit() }],
      ["Check the units: panel sizes are in millimetres (2440, not 244).",
       "Grain: untick “Keep grain direction” where the look allows it, so parts can turn."])}
  {:else}
    {@render stateBlock("Done · no plan", "Not every part fits",
      "The search ended without a way to cut every part from the panels listed.",
      [{ label: "Edit job", primary: true, editor: true, run: () => session.edit() }],
      [
        ...big.map((p) => `A part is larger than every panel: “${p.id}”, ${size(p)} mm${p.grain ? ", which keeps its grain direction" : ""}.`),
        "Too few panels: leave a panel’s Quantity empty for as many as needed.",
        "Settings too strict: “Not allowed” trimming with 2 stages can leave no way to place every part.",
      ])}
  {/if}
{:else if searching && panel === null}
  <!-- Searching: progress, figures, the best plan so far. -->
  <main class:pane-main={pane} class:is-stale={stale}>
    {@render staleBar()}
    {@render missingNote()}
    <div class="heading">
      <div>
        <div class="eyebrow">{name || "Cutting plan"} · {job.state === "queued" ? "waiting" : "searching"}</div>
        <svelte:element this={headingTag} class="headline">{best ? `${headline(instance, best.metrics)} so far` : job.state === "queued" ? "Waiting for a free solver" : "No plan yet"}</svelte:element>
        <p class="muted">
          {#if job.state === "queued"}
            {job.queue_position ? `${job.queue_position === 1 ? "Your job runs next" : `${job.queue_position - 1} ${job.queue_position === 2 ? "job" : "jobs"} ahead of yours`}. ` : ""}The time limit only counts once your search starts.
          {:else}
            Looking for a plan with less waste. The best plan so far is shown and replaced as soon as a better one is found.
          {/if}
        </p>
      </div>
      <div class="actions">
        <button class="button" type="button" onclick={() => void session.stop()} disabled={session.stopping}>
          {session.stopping ? "Stopping…" : job.state === "queued" ? "Cancel" : best ? "Stop and keep this plan" : "Stop"}
        </button>
      </div>
    </div>
    <div class="statusline num">
      <span style="color:var(--ink);font-weight:600">{elapsed.toFixed(1)} s</span> of {job.time_limit} s
      {#if !session.connected}{" "}· <span>Reconnecting…</span>{/if}
    </div>
    <div class="progress" role="progressbar" aria-label="Search time" aria-valuenow={Math.round(elapsed)} aria-valuemin={0} aria-valuemax={job.time_limit}>
      <span style="width:{(elapsed / job.time_limit) * 100}%"></span>
    </div>
    {@render figures()}
    {#if best}
      <div class="live-grid">
        <div class="overview">
          {#each patterns as p, i (i)}
            {@const s = patternStats(p)}
            <section class="viewer">
              <div class="viewer-head"><div><h3>Panel {i + 1} <small>/ {patterns.length}</small>{#if p.repeat > 1}{" "}<small>× {p.repeat}</small>{/if}</h3>
                <small>{s.parts} parts · {percent(s.wasteFraction)} waste</small></div></div>
              <PanelDrawing pattern={p} {kerf} grain={grainOf} width={470} labels="names" nameSize={11} title="Panel {i + 1} of the best plan so far" />
            </section>
          {/each}
        </div>
        <aside class="side">
          <div class="side-head"><h3>Plans found</h3><small>{job.plans_found}</small></div>
          <ul class="plans" style="padding:0 16px 8px" aria-live="polite">
            {#each session.history as h, i (h.number)}
              {@const older = session.history[i + 1]}
              <li><span><b>Plan {h.number}</b> · {h.panels} panels · {percent(h.waste)} waste</span>
                {#if older}<span class="ok">−{((older.waste - h.waste) * 100).toFixed(1)} pt</span>{:else}<span class="muted num">{h.foundAfter.toFixed(2)} s</span>{/if}</li>
            {/each}
          </ul>
          <p class="help" style="border-top:1px solid var(--line);padding-top:10px">You can open the plan now; it updates itself while the search runs.</p>
          <div style="padding:0 16px 16px"><a class="button primary block" href={href({ name: "plan", id, panel: 1, cut: null })}>Open plan</a></div>
        </aside>
      </div>
    {/if}
  </main>
{:else if best}
  {@const total = totalParts(instance)}
  <main class:pane-main={pane} class:is-stale={stale}>
    {@render staleBar()}
    {@render missingNote()}
    <div class="heading">
      <div>
        <div class="eyebrow">{name || "Cutting plan"} · plan {best.number}</div>
        <svelte:element this={headingTag} class="headline">{headline(instance, best.metrics)}</svelte:element>
        <p class="muted">{panelsLine(instance, best.plan)}</p>
      </div>
      <div class="actions">
        {#if !searching}<button class="button" type="button" onclick={() => window.print()}>Print</button>{/if}
        {#if !pane}<button class="button" type="button" onclick={() => session.edit()}>Edit job</button>{/if}
      </div>
    </div>
    {@render figures()}
    <p class="statusline" style="margin:-8px 0 20px" aria-live="polite">
      {#if searching}
        <span>Still searching · {elapsed.toFixed(1)} s of {job.time_limit} s; the plan updates itself.</span>
        <button class="link" type="button" onclick={() => void session.stop()}>{session.stopping ? "Stopping…" : "Stop and keep this plan"}</button>
      {:else}
        <span class="ok">✓ {best.metrics.parts_cut >= total ? "Complete plan" : "Checked plan"}</span>
        {#if job.ended_by === "complete"}
          <span>· the search finished; this is the best plan it can find.</span>
        {:else if job.ended_by === "time_limit"}
          <span>· time limit reached after {job.time_limit} s, so a longer search may still save material.</span>
          {#if job.time_limit < 600}<button class="link" type="button" onclick={() => void session.again(longerSearch)}>Search for {longerSearch} s</button>{/if}
        {:else if job.ended_by === "cancelled"}
          <span>· stopped after {job.elapsed.toFixed(1)} s; this is the best plan found before the stop.</span>
          <button class="link" type="button" onclick={() => void session.again()}>Search again</button>
        {:else if job.state === "failed"}
          <span style="color:var(--bad)">· the search then stopped with an error; this plan was checked and is kept.</span>
        {/if}
      {/if}
    </p>

    {#if pattern && order}
      {@const s = patternStats(pattern)}
      {@const panelInfo = instance.panels.find((p) => p.id === pattern.panel_id)}
      <div class="workbench">
        <div class="wb-nav"><SheetNav {id} {patterns} current={panel} {kerf} grain={grainOf} /></div>
        <div class="wb-main">
          <section class="viewer">
            <div class="viewer-head">
              <div>
                <h2>Panel {panel} <small>/ {patterns.length}</small>{#if pattern.repeat > 1}{" "}<small>× {pattern.repeat} panels</small>{/if}</h2>
                <small>{pattern.panel_id} · {panelInfo ? size(panelInfo) : size(pattern.root)} · {s.parts} parts · {percent(s.wasteFraction)} waste</small>
              </div>
              <small>Grain →</small>
            </div>
            <PanelDrawing {pattern} {kerf} grain={grainOf} width={700} labels="full" rulers cuts={order.cuts} {current} nameSize={11.5}
              title="Panel {panel} of {patterns.length}: {pattern.panel_id}, {mm(pattern.root.length)} by {mm(pattern.root.width)} mm"
              oncut={goCut} onpart={(n) => (selected = selected === n ? null : n)} {selected} />
            <Legend {kerf} />
            <div class="detail" aria-live="polite">
              {#if selected && selected.kind === "part"}
                {@const part = instance.parts.find((p) => p.id === (selected?.kind === "part" ? selected.part_id : ""))}
                <div><small>Selected part</small>
                  <div class="now">{selected.part_id} <em>{part ? size(part) : size(selected)} mm · {part?.grain ? "↔ keeps the grain direction" : "may turn"}{selected.rotated ? " · turned 90°" : ""}</em></div></div>
                <button class="button" type="button" onclick={() => (selected = null)}>Back to cut {current}</button>
              {:else if currentCut}
                <div><small>Cut {current} of {order.cuts.length}</small>
                  <div class="now">{mm(currentCut.measure)} mm <em>{currentCut.direction === "along" ? "along the length" : "across the length"} → {what(currentCut.produces)}</em></div></div>
                <div class="stepper">
                  <button class="button" type="button" aria-label="Previous cut" disabled={current <= 1} onclick={() => goCut(current - 1)}>‹</button>
                  <button class="button primary" type="button" aria-label="Next cut" disabled={current >= order.cuts.length} onclick={() => goCut(current + 1)}>›</button>
                </div>
              {:else}
                <div><small>No cuts</small><div class="now">The part is the whole panel.</div></div>
              {/if}
            </div>
          </section>
        </div>
        <aside class="side wb-cuts" aria-label="Cut order">
          <div class="scroll">
            <div class="side-head"><h3>Cut order</h3><small>{order.cuts.length} cuts</small></div>
            <p class="help">Each size is the piece to cut off, measured from the edge you cut from: set the fence or stop to it.</p>
            <CutList {order} {current} onselect={goCut} />
          </div>
        </aside>
        <div class="wb-below">
          <div class="below" style="margin-top:0">
            <section class="manifest"><h3>On this panel{#if pattern.repeat > 1}{" "}<small>· each of {pattern.repeat}</small>{/if}</h3>
              {#each onPanel as [pid, n] (pid)}
                {@const part = instance.parts.find((p) => p.id === pid)}
                <div class="manifest-row"><div>{pid} <span>{part ? size(part) : ""}</span></div><strong>× {n}</strong></div>
              {/each}
            </section>
            <section class="manifest"><h3>Offcuts <small>· largest first</small></h3>
              {#each offs.slice(0, 5) as o, i (i)}
                <div class="manifest-row"><div>{size(o)}</div><span>{(area(o) / 1e6).toFixed(3)} m²</span></div>
              {:else}
                <p class="help" style="margin-top:8px">No offcuts on this panel.</p>
              {/each}
              {#if offs.length > 5}<p class="help" style="margin-top:8px">{offs.length - 5} smaller offcuts not listed. Slivers thinner than the kerf count as saw dust.</p>{/if}
            </section>
          </div>
          {@render details()}
        </div>
      </div>
    {:else}
      <div class="workbench two" class:bare={pane}>
        {#if !pane}<SheetNav {id} {patterns} current={null} {kerf} grain={grainOf} />{/if}
        <div style="min-width:0">
          <div class="overview">
            {#each patterns as p, i (i)}
              {@const s = patternStats(p)}
              <section class="viewer">
                <div class="viewer-head">
                  <div><h3>Panel {i + 1}{#if p.repeat > 1}{" "}<small>× {p.repeat}</small>{/if}</h3><small>{s.parts} parts · {percent(s.wasteFraction)} waste</small></div>
                  <a class="link" href={href({ name: "plan", id, panel: i + 1, cut: null })}>Cut order →</a>
                </div>
                <PanelDrawing pattern={p} {kerf} grain={grainOf} width={540} labels="full" nameSize={10.5} title="Panel {i + 1}: {p.panel_id}" />
              </section>
            {/each}
            <section class="viewer">
              <div class="viewer-head"><div><h3>Parts checklist</h3><small>{best.metrics.parts_cut} of {total} included</small></div></div>
              <div class="table-wrap">
                <table>
                  <thead><tr><th>Part</th><th class="num">Length, mm</th><th class="num">Width, mm</th><th class="c">Grain</th><th class="num">Needed</th><th class="num">Cut</th><th>Panels</th></tr></thead>
                  <tbody>
                    {#each checklistRows as r (r.part.id)}
                      <tr class:not-cut={r.cut < r.needed}>
                        <td>{r.part.id}</td><td class="num">{mm(r.part.length)}</td><td class="num">{mm(r.part.width)}</td>
                        <td class="c">{r.part.grain ? "↔" : "—"}</td><td class="num">{r.needed}</td>
                        <td class="num">{#if r.cut >= r.needed}<span class="ok">✓</span>{:else}not cut{/if} {r.cut}</td>
                        <td>{r.panels.join(", ")}</td>
                      </tr>
                    {/each}
                  </tbody>
                </table>
              </div>
            </section>
          </div>
          <Legend {kerf} />
          {@render details()}
        </div>
      </div>
    {/if}
  </main>
  {#if !searching}<PrintSheets {job} {instance} {best} grain={grainOf} />{/if}
{:else}
  {@render stateBlock("Searching", "Waiting for the first plan", "The first plan usually appears within a second.", [])}
{/if}
