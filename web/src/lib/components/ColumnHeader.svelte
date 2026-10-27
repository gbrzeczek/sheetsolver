<!-- A table column header with its help (?) button and popover, or, for a
     self-explanatory column, its help only for screen readers (DESIGN.md §5). -->
<script lang="ts">
  import { field, hasHelpButton, helpHtml, type FieldKey } from "../api/fields";

  interface Props {
    key: FieldKey;
    /** Shown instead of the label, e.g. "Keep grain"; the label stays the accessible name. */
    short?: string;
    align?: "left" | "right";
    class?: string;
    focused?: boolean;
  }
  let { key, short, align = "left", class: cls = "", focused = false }: Props = $props();

  const f = $derived(field(key));
  const id = $derived(`help-${key.replace(".", "-")}`);
  let open = $state(false);
  let root: HTMLElement | undefined = $state();

  function outside(event: Event) {
    if (open && root && !root.contains(event.target as Node)) open = false;
  }
  function key_(event: KeyboardEvent) {
    if (open && event.key === "Escape") open = false;
  }
</script>

<svelte:window onpointerdown={outside} onkeydown={key_} />

<th class="{cls}{focused ? ' focus-col' : ''}" scope="col" bind:this={root}>
  <span aria-hidden={short ? "true" : undefined}>{short ?? f.label}</span>{#if short}<span class="visually-hidden">{f.label}</span>{/if}{#if hasHelpButton(key)}<button
      class="help-btn"
      type="button"
      aria-expanded={open}
      aria-controls={id}
      aria-label="About {f.label}"
      onclick={() => (open = !open)}>?</button
    >
    <div class="pop {align}" {id} role="dialog" aria-label={f.label} hidden={!open}>
      <div class="pop-title">
        <span>{f.label}</span>
        <button class="icon-btn" style="width:18px;height:18px" aria-label="Close" onclick={() => (open = false)}>×</button>
      </div>
      <!-- eslint-disable-next-line svelte/no-at-html-tags -- helpHtml escapes everything but its own tags -->
      {@html helpHtml(f.help)}
    </div>
  {:else}
    <!-- eslint-disable-next-line svelte/no-at-html-tags -- helpHtml escapes everything but its own tags -->
    <span class="help-only" {id} hidden>{@html helpHtml(f.help)}</span>
  {/if}
</th>
