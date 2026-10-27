<!-- A settings field: label, help (?) button, the control, and the help
     text expanded inline under it (DESIGN.md §5). A self-explanatory field
     has no button; its help is only read by screen readers. -->
<script lang="ts">
  import type { Snippet } from "svelte";
  import { field, hasHelpButton, helpHtml, type FieldKey } from "../api/fields";

  interface Props {
    key: FieldKey;
    /** The option to emphasise in the help text. */
    current?: string;
    optional?: boolean;
    error?: string;
    /** The id of the control, so the label points at it. */
    for?: string;
    children: Snippet<[{ helpId: string; label: string }]>;
  }
  let { key, current, optional = false, error, for: forId, children }: Props = $props();

  const f = $derived(field(key));
  const helpId = $derived(`help-${key.replace(".", "-")}`);
  let open = $state(false);
</script>

<div class="field">
  <div class="label-row">
    <label class="field-label" for={forId}>{f.label}{#if optional}{" "}<span class="unit">optional</span>{/if}</label>
    {#if hasHelpButton(key)}
      <button
        class="help-btn"
        type="button"
        aria-expanded={open}
        aria-controls={helpId}
        aria-label="About {f.label}"
        onclick={() => (open = !open)}>?</button
      >
    {/if}
  </div>
  {@render children({ helpId, label: f.label })}
  {#if error}<div class="cell-error" role="alert"><span>{error}</span></div>{/if}
  <!-- Always in the page, so screen readers read it with the field. -->
  <div class="help-box" id={helpId} hidden={!open}>
    <!-- eslint-disable-next-line svelte/no-at-html-tags -- helpHtml escapes everything but its own tags -->
    {@html helpHtml(f.help, current)}
  </div>
</div>
