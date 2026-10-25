<!-- "Parts to make": an editable table (DESIGN.md §2, Job setup). -->
<script lang="ts">
  import ColumnHeader from "./ColumnHeader.svelte";
  import { emptyPart, namesFor, type PartRow } from "../job/draft";
  import { field, type FieldKey } from "../api/fields";

  interface Props {
    rows: PartRow[];
    problems: Map<string, string>;
    showValue: boolean;
    onedit(path: string): void;
    onremove(index: number): void;
  }
  let { rows = $bindable(), problems, showValue, onedit, onremove }: Props = $props();

  const names = $derived(namesFor(rows, "Part"));
  let focusedCol = $state<string | null>(null);

  type Col = { name: "id" | "length" | "width" | "quantity" | "value"; cls: string; unit?: string; numeric: boolean };
  const cols = $derived<Col[]>([
    { name: "id", cls: "w-name", numeric: false },
    { name: "length", cls: "w-len num", unit: "mm", numeric: true },
    { name: "width", cls: "w-len num", unit: "mm", numeric: true },
    { name: "quantity", cls: "w-qty num", numeric: true },
    ...(showValue ? [{ name: "value", cls: "w-qty num", numeric: true } as Col] : []),
  ]);

  const path = (i: number, name: string) => `instance.parts[${i}].${name}`;

  function add() {
    rows.push(emptyPart());
    queueMicrotask(() => document.querySelector<HTMLInputElement>(`#part-${rows.length - 1}-id`)?.focus());
  }

  function keydown(event: KeyboardEvent, i: number) {
    if (event.key === "Enter" && i === rows.length - 1) {
      event.preventDefault();
      add();
    }
  }
</script>

<div class="table-wrap" style="overflow:visible">
  <table>
    <thead>
      <tr>
        <th><span class="visually-hidden">Row</span></th>
        {#each cols as col (col.name)}
          <ColumnHeader key={`Part.${col.name}` as FieldKey} unit={col.unit} class={col.cls} focused={focusedCol === col.name} />
        {/each}
        <ColumnHeader key="Part.grain" short="Keep grain ↔" class="w-chk c" align="right" focused={focusedCol === "grain"} />
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
                id="part-{i}-{col.name}"
                class="cell"
                class:text={!col.numeric}
                class:invalid={problem}
                aria-invalid={problem ? "true" : undefined}
                aria-describedby={problem ? `err-part-${i}-${col.name}` : `help-Part-${col.name}`}
                aria-label="{field(`Part.${col.name}` as FieldKey).label}, row {i + 1}"
                inputmode={col.numeric ? (col.name === "quantity" ? "numeric" : "decimal") : undefined}
                autocomplete="off"
                placeholder={col.name === "id" ? names[i] : col.name === "quantity" ? "1" : col.name === "value" ? "Area" : ""}
                bind:value={row[col.name]}
                oninput={() => onedit(path(i, col.name))}
                onfocus={() => (focusedCol = col.name)}
                onblur={() => (focusedCol = null)}
                onkeydown={(e) => keydown(e, i)}
              />
            </td>
          {/each}
          <td class="c">
            <input
              type="checkbox"
              aria-label="{field('Part.grain').label}, row {i + 1}"
              aria-describedby="help-Part-grain"
              bind:checked={row.grain}
              onfocus={() => (focusedCol = "grain")}
              onblur={() => (focusedCol = null)}
              onkeydown={(e) => keydown(e, i)}
            />
          </td>
          <td>
            <button class="icon-btn" type="button" aria-label="Remove row {i + 1}" onclick={() => onremove(i)}>×</button>
          </td>
        </tr>
        {#each rowProblems as col (col.name)}
          <tr class="err-row">
            <td></td>
            <td colspan={cols.length + 2}>
              <div class="cell-error" id="err-part-{i}-{col.name}">
                <strong>{field(`Part.${col.name}` as FieldKey).label}:</strong>
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
  <button class="link" type="button" onclick={add}>+ Add part</button>
  <small>↔ The part keeps the panel’s grain direction.</small>
</div>
