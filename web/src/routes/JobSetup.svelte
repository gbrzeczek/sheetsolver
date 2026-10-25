<!-- Job setup: "What are you cutting?" (DESIGN.md §2, mockups/editor.html). -->
<script lang="ts">
  import Setting from "../lib/components/Setting.svelte";
  import PartsTable from "../lib/components/PartsTable.svelte";
  import PanelsTable from "../lib/components/PanelsTable.svelte";
  import PlanSettings from "../lib/components/PlanSettings.svelte";
  import { session } from "../lib/job/session.svelte";
  import { byPath, place } from "../lib/job/problems";
  import { emptyPanel, emptyPart, type PanelRow, type PartRow } from "../lib/job/draft";

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
    const ok = await session.submit();
    if (!ok && session.problems.length) {
      document.querySelector(".alert")?.scrollIntoView({ behavior: "smooth", block: "center" });
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

<form class="narrow" onsubmit={submit} novalidate>
  <div class="heading">
    <div>
      <div class="eyebrow">Job setup</div>
      <h1>What are you cutting?</h1>
      <p class="muted">
        Sizes in millimetres, at most one decimal (720, 3.2, 18.5). Tap <span class="help-btn" aria-hidden="true" style="display:inline-grid">?</span>
        for what a field means.
      </p>
    </div>
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

  <div class="setup-grid">
    <div>
      <div class="job-name">
        <Setting key="JobRequest.name" optional for="job-name" error={problems.get("name")}>
          {#snippet children({ helpId })}
            <input id="job-name" class="input" placeholder="Untitled job" autocomplete="off" aria-describedby={helpId}
              class:invalid={problems.has("name")} maxlength={200}
              bind:value={session.draft.name} oninput={() => edited("name")} />
          {/snippet}
        </Setting>
      </div>

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
          onedit={edited}
          onremove={(i) => remove("panels", i)}
        />
      </section>
    </div>

    <aside class="setup-aside">
      <PlanSettings bind:draft={session.draft} {problems} onedit={edited} />
      <div class="summary-row" style="margin-top:6px"><span>Parts</span><strong>{partCount} in {sizes} sizes</strong></div>
      <div class="aside-cta hide-tablet">
        <button class="button primary big block" type="submit" disabled={session.submitting || retryIn > 0}>
          {session.submitting ? "Sending…" : retryIn > 0 ? `Solver busy, try again in ${retryIn} s` : "Find cutting plan →"}
        </button>
        <p class="hint" style="margin-top:12px" class:bad={session.problems.length > 0}>
          {session.problems.length ? `${session.problems.length} problems need attention.` : "Empty names are filled in for you."}
        </p>
      </div>
    </aside>
  </div>

  <div class="cta-bar show-tablet">
    <small>{partCount} parts · {draft.panels.length} panel {draft.panels.length === 1 ? "size" : "sizes"}</small>
    <button class="button primary big" type="submit" disabled={session.submitting || retryIn > 0}>
      {retryIn > 0 ? `Try again in ${retryIn} s` : "Find cutting plan →"}
    </button>
  </div>
</form>

{#if undo}
  <div class="toast" role="status">Row removed. <button class="link" type="button" onclick={restore}>Undo</button></div>
{/if}

<style>
  .bad {
    color: var(--bad) !important;
  }
</style>
