<!-- "Panels you have": an editable table (DESIGN.md §2, Job setup). -->
<script lang="ts">
  import ColumnHeader from "./ColumnHeader.svelte";
  import { emptyPanel, namesFor, type PanelRow } from "../job/draft";
  import { field, type FieldKey } from "../api/fields";

  interface Props {
    rows: PanelRow[];
    problems: Map<string, string>;
    /** "Shortest length/width" goals take exactly one panel. */
    single: boolean;
    onedit(path: string): void;
    onremove(index: number): void;
  }
  let { rows = $bindable(), problems, single, onedit, onremove }: Props = $props();

  const names = $derived(namesFor(rows, "Panel"));
  let focusedCol = $state<string | null>(null);

  const cols = [
    { name: "id", cls: "w-name", numeric: false, placeholder: "" },
    { name: "length", cls: "w-len num", unit: "mm", numeric: true, placeholder: "" },
    { name: "width", cls: "w-len num", unit: "mm", numeric: true, placeholder: "" },
    { name: "quantity", cls: "w-qty-wide num", numeric: true, placeholder: "Unlimited" },
    { name: "cost", cls: "w-qty num", numeric: true, placeholder: "Area" },
  ] as const;

  const path = (i: number, name: string) => `instance.panels[${i}].${name}`;

  function add() {
    rows.push(emptyPanel());
    queueMicrotask(() => document.querySelector<HTMLInputElement>(`#panel-${rows.length - 1}-id`)?.focus());
  }
</script>

<div class="table-wrap" style="overflow:visible">
  <table>
    <thead>
      <tr>
        <th><span class="visually-hidden">Row</span></th>
        {#each cols as col (col.name)}
          <ColumnHeader
            key={`Panel.${col.name}` as FieldKey}
            unit={"unit" in col ? col.unit : undefined}
            class={col.cls}
            align={col.name === "cost" || col.name === "quantity" ? "right" : "left"}
            focused={focusedCol === col.name}
          />
        {/each}
        <th class="w-del"><span class="visually-hidden">Remove</span></th>
      </tr>
    </thead>
    <tbody>
      {#each rows as row, i (row.key)}
        {@const rowProblems = cols.filter((c) => problems.has(path(i, c.name)))}
        <tr class:has-err={rowProblems.length > 0}>
          <td class="idx">{i + 1}</td>
          {#each cols as col (col.name)}
            {@const problem = problems.get(path(i, col.name))}
            <td>
              <input
                id="panel-{i}-{col.name}"
                class="cell"
                class:text={!col.numeric}
                class:invalid={problem}
                aria-invalid={problem ? "true" : undefined}
                aria-describedby={problem ? `err-panel-${i}-${col.name}` : `help-Panel-${col.name}`}
                aria-label="{field(`Panel.${col.name}` as FieldKey).label}, panel {i + 1}"
                inputmode={col.numeric ? (col.name === "quantity" ? "numeric" : "decimal") : undefined}
                autocomplete="off"
                placeholder={col.name === "id" ? names[i] : col.placeholder}
                bind:value={row[col.name]}
                oninput={() => onedit(path(i, col.name))}
                onfocus={() => (focusedCol = col.name)}
                onblur={() => (focusedCol = null)}
              />
            </td>
          {/each}
          <td>
            <button class="icon-btn" type="button" aria-label="Remove panel {i + 1}" onclick={() => onremove(i)}>×</button>
          </td>
        </tr>
        {#each rowProblems as col (col.name)}
          <tr class="err-row">
            <td></td>
            <td colspan={cols.length + 1}>
              <div class="cell-error" id="err-panel-{i}-{col.name}">
                <strong>{field(`Panel.${col.name}` as FieldKey).label}:</strong>
                {problems.get(path(i, col.name))}
              </div>
            </td>
          </tr>
        {/each}
      {/each}
    </tbody>
  </table>
</div>
<div class="table-foot">
  <button class="link" type="button" onclick={add} disabled={single && rows.length >= 1}>+ Add panel size</button>
  <small>
    {single ? "This goal cuts from exactly one panel, quantity 1." : "Leave Quantity empty for as many as needed."}
  </small>
</div>
