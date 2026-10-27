<!-- A panel's trims (docs/FIELDS.md, *Panel trims*): collapsed under its
     row with a summary of the values inside (DESIGN.md §10), open when one
     of them has a problem. One line per edge: its *Trim* width and its
     *Trimmed strip*. -->
<script lang="ts">
  import ColumnHeader from "./ColumnHeader.svelte";
  import HelpText from "./HelpText.svelte";
  import { describedBy, field, helpId, options } from "../api/fields";
  import { EDGES, type Edge, type TrimRow } from "../job/draft";
  import { EDGE_NAMES } from "../job/problems";
  import { trimsSummary } from "../job/trims";
  import type { TrimCut } from "../api/types";

  interface Props {
    trims: Record<Edge, TrimRow>;
    /** The panel's row, 0-based, for ids and problem paths. */
    index: number;
    problems: Map<string, string>;
    onedit(path: string): void;
  }
  let { trims = $bindable(), index, problems, onedit }: Props = $props();

  const path = (edge: Edge, name: "width" | "cut") => `instance.panels[${index}].trims.${edge}.${name}`;
  const hasProblem = $derived(EDGES.some((edge) => problems.has(path(edge, "width")) || problems.has(path(edge, "cut"))));
  let open = $state(false);
  $effect(() => {
    if (hasProblem) open = true;
  });

  const summary = $derived(trimsSummary(trims));
  const helpKey = "Panel.trims" as const;
  let helpOpen = $state(false);
  // Each panel has its own editor: ids get the panel's row, but the first
  // panel's keep the plain ones.
  const suffix = $derived(index ? `-${index}` : "");
  const trimsHelp = $derived(helpId(helpKey) + suffix);
  const described = (key: "Trim.width" | "Trim.cut") => (suffix ? `${helpId(key)}${suffix}-short` : describedBy(key));
</script>

<details class="trims" bind:open>
  <summary>
    <span class="trims-title">Trims</span>
    <span class="trims-sum">{summary}</span>
  </summary>
  <div class="trims-body">
    <div class="label-row trims-label">
      <span class="field-label" id="trims-{index}-label">{field(helpKey).label}<span class="visually-hidden">, panel {index + 1}</span></span>
      <button
        class="help-btn"
        type="button"
        aria-expanded={helpOpen}
        aria-controls={trimsHelp}
        aria-label="About {field(helpKey).label}"
        onclick={() => (helpOpen = !helpOpen)}>?</button
      >
    </div>
    <div class="help-box" id={trimsHelp} hidden={!helpOpen}>
      <HelpText key={helpKey} id={trimsHelp} />
    </div>
    <table class="trims-table" aria-labelledby="trims-{index}-label">
      <thead>
        <tr>
          <th scope="col"><span class="visually-hidden">Edge</span></th>
          <ColumnHeader key="Trim.width" class="w-len num" align="left" {suffix} />
          <ColumnHeader key="Trim.cut" align="right" {suffix} />
        </tr>
      </thead>
      <tbody>
        {#each EDGES as edge (edge)}
          {@const widthProblem = problems.get(path(edge, "width"))}
          <tr class:has-err={widthProblem}>
            <th scope="row" class="edge">{EDGE_NAMES[edge]}</th>
            <td>
              <input
                  id="panel-{index}-trim-{edge}"
                  class="cell"
                  class:invalid={widthProblem}
                  inputmode="decimal"
                  autocomplete="off"
                  placeholder="0"
                  aria-label="{field('Trim.width').label}, {EDGE_NAMES[edge]}, panel {index + 1}"
                  aria-invalid={widthProblem ? "true" : undefined}
                  aria-describedby={widthProblem ? `err-panel-${index}-trim-${edge}` : described("Trim.width")}
                  bind:value={trims[edge].width}
                  oninput={() => onedit(path(edge, "width"))}
                />
            </td>
            <td>
              <select
                id="panel-{index}-trim-{edge}-cut"
                class="select cell-select"
                aria-label="{field('Trim.cut').label}, {EDGE_NAMES[edge]}, panel {index + 1}"
                aria-describedby={described("Trim.cut")}
                bind:value={trims[edge].cut}
                onchange={() => onedit(path(edge, "cut"))}
              >
                {#each options("Trim.cut") as option (option.value)}
                  <option value={option.value as TrimCut}>{option.title}</option>
                {/each}
              </select>
            </td>
          </tr>
          {#if widthProblem}
            <tr class="err-row">
              <td></td>
              <td colspan="2"><div class="cell-error" id="err-panel-{index}-trim-{edge}">{widthProblem}</div></td>
            </tr>
          {/if}
        {/each}
      </tbody>
    </table>
  </div>
</details>
