<!-- Job setup: "What are you cutting?" (DESIGN.md §2, mockups/editor.html).
     A page of its own below 1280 px; the left column of the workbench above. -->
<script lang="ts">
  import Setting from "../lib/components/Setting.svelte";
  import PartsTable from "../lib/components/PartsTable.svelte";
  import PanelsTable from "../lib/components/PanelsTable.svelte";
  import PlanSettings from "../lib/components/PlanSettings.svelte";
  import { session } from "../lib/job/session.svelte";
  import { byPath, place } from "../lib/job/problems";
  import { emptyPanel, emptyPart, newDraft, type Draft, type PanelRow, type PartRow } from "../lib/job/draft";

  interface Props {
    /** In the workbench: one column, with the plan beside it. */
    pane?: boolean;
  }
  let { pane = false }: Props = $props();

  const problems = $derived(byPath(session.problems));
  const pageProblems = $derived(session.problems.filter((p) => place(p.field).section === "page"));
  const partsProblem = $derived(problems.get("instance.parts"));
  const panelsProblem = $derived(problems.get("instance.panels"));
  const draft = $derived(session.draft);
  const openDimension = $derived(draft.objective === "open_dimension_x" || draft.objective === "open_dimension_y");
  const partCount = $derived(
    draft.parts.reduce((sum, p) => sum + (p.length.trim() || p.width.trim() ? Number(p.quantity) || 1 : 0), 0),
  );
  const sizes = $derived(draft.parts.filter((p) => p.length.trim() || p.width.trim()).length);
  // A cost already typed, or a problem with one, keeps the column on screen,
  // so nothing that is sent is out of sight.
  const showCost = $derived(
    draft.panels.length >= 2 || draft.panels.some((p, i) => p.cost.trim() || problems.has(`instance.panels[${i}].cost`)),
  );

  // Remove a row with Undo instead of a confirmation.
  let undo = $state<{ list: "parts" | "panels"; index: number; row: PartRow | PanelRow; timer: number } | null>(null);
  function remove(list: "parts" | "panels", index: number) {
    if (undo) clearTimeout(undo.timer);
    const rows = list === "parts" ? draft.parts : draft.panels;
    const [row] = rows.splice(index, 1);
    if (rows.length === 0) (rows as unknown[]).push(list === "parts" ? emptyPart() : emptyPanel());
    session.problems = [];
    edited();
    undo = { list, index, row: row!, timer: window.setTimeout(() => (undo = null), 6000) };
  }
  // "New job": start over, with Undo instead of a confirmation.
  let previous = $state<{ draft: Draft; timer: number } | null>(null);
  function startOver() {
    if (previous) clearTimeout(previous.timer);
    previous = { draft: $state.snapshot(session.draft) as Draft, timer: window.setTimeout(() => (previous = null), 8000) };
    session.draft = newDraft();
    session.problems = [];
    edited();
  }
  function bringBack() {
    if (!previous) return;
    clearTimeout(previous.timer);
    session.draft = previous.draft;
    previous = null;
    edited();
  }

  function restore() {
    if (!undo) return;
    const rows = (undo.list === "parts" ? draft.parts : draft.panels) as (PartRow | PanelRow)[];
    rows.splice(undo.index, 0, undo.row);
    clearTimeout(undo.timer);
    undo = null;
    edited();
  }

  function edited(path?: string) {
    if (path) session.touch(path);
    session.save();
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    // ⌘/Ctrl + Enter submits even while the button is disabled.
    if (session.submitting || retryIn > 0) return;
    const ok = await session.submit();
    if (!ok && session.problems.length) {
      document.querySelector(".alert")?.scrollIntoView({ behavior: "smooth", block: "center" });
    }
  }

  // ⌘/Ctrl + Enter finds the plan from any field of the job (DESIGN.md §2).
  let form = $state<HTMLFormElement>();
  function shortcut(event: KeyboardEvent) {
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey) && form?.contains(event.target as Node)) {
      event.preventDefault();
      form.requestSubmit();
    }
  }

  function goTo(path: string | null) {
    const where = place(path);
    if (where.section === "parts" || where.section === "panels") {
      const list = where.section === "parts" ? "part" : "panel";
      const name = where.field?.split(".")[1];
      const input = document.getElementById(`${list}-${where.row}-${name}`) as HTMLInputElement | null;
      input?.focus();
    } else if (where.field) {
      const id = { "Instance.objective": "objective", "Saw.kerf": "kerf", "JobRequest.time_limit": "time-limit", "JobRequest.name": "job-name" }[where.field as string];
      if (id) document.getElementById(id)?.focus();
    }
  }

  let retryIn = $state(0);
  $effect(() => {
    const trouble = session.submitTrouble;
    if (trouble?.kind !== "busy") return;
    const tick = () => (retryIn = Math.max(0, Math.ceil((trouble.retryAt - Date.now()) / 1000)));
    tick();
    const timer = setInterval(tick, 500);
    return () => clearInterval(timer);
  });
</script>

<svelte:window onkeydown={shortcut} />

<form class={pane ? "pane-form" : "narrow"} bind:this={form} onsubmit={submit} novalidate>
  <div class="heading">
    <div>
      <div class="eyebrow">Job setup</div>
      <h1>What are you cutting?</h1>
      <p class="muted">Sizes in millimetres, at most one decimal (720, 3.2, 18.5).</p>
    </div>
    <div class="actions"><button class="button" type="button" onclick={startOver}>New job</button></div>
  </div>

  {#if session.problems.length}
    <div class="alert" role="alert">
      <h3>{session.problemsMessage ?? "The job cannot be solved as entered."}</h3>
      <p>Fix the highlighted fields, then find the plan again.</p>
      <ul>
        {#each session.problems as problem, i (i)}
          <li>
            {#if problem.field}
              <a href={"#/"} onclick={(e) => { e.preventDefault(); goTo(problem.field); }}>{place(problem.field).label}</a>:
            {/if}
            {problem.message}
          </li>
        {/each}
      </ul>
    </div>
  {/if}
  {#if pageProblems.length === 0 && session.submitTrouble?.kind === "unreachable"}
    <div class="alert" role="alert"><h3>The plan could not be requested.</h3><p>{session.submitTrouble.message}</p></div>
  {/if}

  <div class="setup-grid" class:stacked={pane}>
    <div>
      <section class="form-block" aria-labelledby="parts-h">
        <div class="section-title">
          <h2 id="parts-h">Parts to make</h2>
          <small>{partCount} {partCount === 1 ? "part" : "parts"} · {sizes} {sizes === 1 ? "size" : "sizes"}</small>
        </div>
        {#if partsProblem}<p class="cell-error" style="margin-bottom:8px">{partsProblem}</p>{/if}
        <PartsTable
          bind:rows={session.draft.parts}
          {problems}
          showValue={draft.objective === "knapsack"}
          onedit={edited}
          onremove={(i) => remove("parts", i)}
        />
      </section>

      <section class="form-block" aria-labelledby="panels-h">
        <div class="section-title">
          <h2 id="panels-h">Panels you have</h2>
          <small>{draft.panels.length} {draft.panels.length === 1 ? "size" : "sizes"}</small>
        </div>
        {#if panelsProblem}<p class="cell-error" style="margin-bottom:8px">{panelsProblem}</p>{/if}
        <PanelsTable
          bind:rows={session.draft.panels}
          {problems}
          single={openDimension}
          {showCost}
          onedit={edited}
          onremove={(i) => remove("panels", i)}
        />
      </section>
    </div>

    <aside class="setup-aside">
      <PlanSettings bind:draft={session.draft} {problems} onedit={edited} />
      {#if !pane}
        <div class="aside-cta hide-tablet">
          <button class="button primary big block" type="submit" disabled={session.submitting || retryIn > 0}>
            {session.submitting ? "Sending…" : retryIn > 0 ? `Solver busy, try again in ${retryIn} s` : "Find cutting plan →"}
          </button>
          {#if session.problems.length}
            <p class="hint bad" style="margin-top:12px">{session.problems.length} {session.problems.length === 1 ? "problem needs" : "problems need"} attention.</p>
          {/if}
        </div>
      {/if}
    </aside>
  </div>

  <div class="cta-bar" class:show-tablet={!pane}>
    <small>{partCount} parts · {draft.panels.length} panel {draft.panels.length === 1 ? "size" : "sizes"}</small>
    <button class="button primary big" type="submit" disabled={session.submitting || retryIn > 0}>
      {session.submitting ? "Sending…" : retryIn > 0 ? `Try again in ${retryIn} s` : "Find cutting plan →"}
      {#if pane && !session.submitting && retryIn === 0}<kbd aria-hidden="true">{navigator.platform.startsWith("Mac") ? "⌘" : "Ctrl"} ↵</kbd>{/if}
    </button>
  </div>
</form>

{#if previous}
  <div class="toast" role="status">New job started. <button class="link" type="button" onclick={bringBack}>Undo</button></div>
{:else if undo}
  <div class="toast" role="status">Row removed. <button class="link" type="button" onclick={restore}>Undo</button></div>
{/if}

<style>
  .bad {
    color: var(--bad) !important;
  }
</style>
