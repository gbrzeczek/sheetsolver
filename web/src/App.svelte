<!-- The app: bar, then the workbench (job and plan side by side, from
     1280 px) or the two tabs and the page of the current route (DESIGN.md §2). -->
<script lang="ts">
  import JobSetup from "./routes/JobSetup.svelte";
  import CuttingPlan from "./routes/CuttingPlan.svelte";
  import { router, href } from "./lib/router.svelte";
  import { layout } from "./lib/layout.svelte";
  import { session } from "./lib/job/session.svelte";
  import { isFinal } from "./lib/api/types";

  const route = $derived(router.route);
  const live = $derived(session.job !== null && !isFinal(session.job.state));
  const planId = $derived(route.name === "plan" ? route.id : session.jobId);
  // The one-panel saw view is full screen at every width; anything else is
  // the workbench on a wide window.
  const bench = $derived(layout.wide && (route.name === "setup" || route.panel === null));
  const context = $derived(
    route.name === "plan" && !bench ? session.jobName || "Untitled job" : session.draft.name.trim() || "Untitled job",
  );

  $effect(() => {
    document.title = route.name === "setup" && !bench ? "Job setup · Sheetsolver" : `${context} · Sheetsolver`;
  });
</script>

<header class="chrome">
  <span class="brand">
    <svg width="18" height="18" viewBox="0 0 18 18" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.6">
      <rect x="1.5" y="1.5" width="15" height="15" rx="1.5" /><path d="M1.5 9.5h15M8 1.5v8M11.5 9.5v7" />
    </svg>Sheetsolver
  </span>
  <span class="context">{context}</span>
</header>
{#if !bench}
  <nav class="topnav" aria-label="Job views">
    <a href="#/" aria-current={route.name === "setup" ? "page" : undefined}>Job setup</a>
    {#if planId}
      <a href={href({ name: "plan", id: planId, panel: null, cut: null })} aria-current={route.name === "plan" ? "page" : undefined}>
        Cutting plan{#if live}<span class="live" aria-label="searching"></span>{/if}
      </a>
    {:else}
      <a aria-disabled="true" style="opacity:.5;pointer-events:none">Cutting plan</a>
    {/if}
  </nav>
{/if}

{#if bench}
  <div class="bench" class:with-value={session.draft.objective === "knapsack"}>
    <div class="bench-job"><JobSetup pane /></div>
    <div class="bench-plan" aria-label="Cutting plan" role="region">
      {#if planId}
        {#key planId}
          <CuttingPlan id={planId} panel={null} cut={null} pane />
        {/key}
      {:else}
        <section class="state bench-empty">
          <div class="eyebrow">Cutting plan</div>
          <h2>No plan yet</h2>
          <p>Enter the parts and the panels you have, then choose <strong>Find cutting plan</strong>. The plan appears here, next to the job, so you can change the job and compare.</p>
        </section>
      {/if}
    </div>
  </div>
{:else if route.name === "setup"}
  <JobSetup />
{:else}
  {#key route.id}
    <CuttingPlan id={route.id} panel={route.panel} cut={route.cut} />
  {/key}
{/if}

{#if session.replaced}
  <div class="toast" role="status">The plan's job is in the editor. <button class="link" type="button" onclick={() => session.undoEdit()}>Undo</button></div>
{/if}
