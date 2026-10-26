<!-- "Plan settings": Goal and kerf always visible, the rest collapsed with a
     summary of their values (DESIGN.md §2). -->
<script lang="ts">
  import Setting from "./Setting.svelte";
  import Choice from "./Choice.svelte";
  import { optionTitle, options } from "../api/fields";
  import type { Draft } from "../job/draft";
  import type { CutType, FirstCuts, Objective, Stages } from "../api/types";

  interface Props {
    draft: Draft;
    problems: Map<string, string>;
    onedit(path: string): void;
  }
  let { draft = $bindable(), problems, onedit }: Props = $props();

  const advancedProblem = $derived(
    ["instance.saw.stages", "instance.saw.cut_type", "instance.saw.first_cuts", "time_limit"].some((p) => problems.has(p)),
  );
  let advancedOpen = $state(false);
  $effect(() => {
    if (advancedProblem) advancedOpen = true;
  });

  const summary = $derived(
    [
      draft.stages === "unlimited" ? "unlimited stages" : `${draft.stages} stages`,
      draft.stages === "unlimited" ? null : `trimming ${optionTitle("Saw.cut_type", draft.cutType).toLowerCase()}`,
      draft.firstCuts === "either" ? "first cuts either way" : `${optionTitle("Saw.first_cuts", draft.firstCuts).toLowerCase()} first`,
      `${draft.timeLimit || "?"} s`,
    ]
      .filter(Boolean)
      .join(" · "),
  );
</script>

<h2 style="margin-bottom:16px">Plan settings</h2>

<Setting key="Instance.objective" current={optionTitle("Instance.objective", draft.objective)} for="objective" error={problems.get("instance.objective")}>
  {#snippet children({ helpId })}
    <select
      id="objective"
      class="select"
      aria-describedby={helpId}
      bind:value={draft.objective}
      onchange={() => onedit("instance.objective")}
    >
      {#each options("Instance.objective") as option (option.value)}
        <option value={option.value as Objective}>{option.title}</option>
      {/each}
    </select>
  {/snippet}
</Setting>

<Setting key="Saw.kerf" for="kerf" error={problems.get("instance.saw.kerf")}>
  {#snippet children({ helpId })}
    <div class="input-wrap input-short">
      <input
        id="kerf"
        class="input"
        class:invalid={problems.has("instance.saw.kerf")}
        inputmode="decimal"
        autocomplete="off"
        aria-describedby={helpId}
        bind:value={draft.kerf}
        oninput={() => onedit("instance.saw.kerf")}
      /><span class="suffix">mm</span>
    </div>
  {/snippet}
</Setting>

<details class="advanced" bind:open={advancedOpen}>
  <summary><span class="sum">More cutting settings <small>{summary}</small></span></summary>
  <div class="inner">
    <Setting key="Saw.stages" error={problems.get("instance.saw.stages")}>
      {#snippet children({ helpId })}
        <Choice key="Saw.stages" name="stages" value={draft.stages} describedby={helpId}
          onchange={(v) => { draft.stages = v as Stages; onedit("instance.saw.stages"); }} />
      {/snippet}
    </Setting>
    <Setting key="Saw.cut_type" current={optionTitle("Saw.cut_type", draft.cutType)} error={problems.get("instance.saw.cut_type")}>
      {#snippet children({ helpId })}
        <Choice key="Saw.cut_type" name="cut_type" value={draft.cutType} describedby={helpId}
          disabled={draft.stages === "unlimited"}
          onchange={(v) => { draft.cutType = v as CutType; onedit("instance.saw.cut_type"); }} />
      {/snippet}
    </Setting>
    <Setting key="Saw.first_cuts" current={optionTitle("Saw.first_cuts", draft.firstCuts)} error={problems.get("instance.saw.first_cuts")}>
      {#snippet children({ helpId })}
        <Choice key="Saw.first_cuts" name="first_cuts" value={draft.firstCuts} describedby={helpId}
          onchange={(v) => { draft.firstCuts = v as FirstCuts; onedit("instance.saw.first_cuts"); }} />
      {/snippet}
    </Setting>
    <Setting key="JobRequest.time_limit" for="time-limit" error={problems.get("time_limit")}>
      {#snippet children({ helpId })}
        <div class="input-wrap input-short">
          <input
            id="time-limit"
            class="input"
            class:invalid={problems.has("time_limit")}
            inputmode="numeric"
            autocomplete="off"
            aria-describedby={helpId}
            bind:value={draft.timeLimit}
            oninput={() => onedit("time_limit")}
          /><span class="suffix">s</span>
        </div>
      {/snippet}
    </Setting>
  </div>
</details>
