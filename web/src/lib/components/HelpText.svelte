<!-- A field's two levels of help (DESIGN.md §5): the short help first, and
     *More* for the full text, which then takes its place. Inside a help box
     or a column's popover; both stay in the page when closed, so the short
     help (`{id}-short`) can be read with its input by screen readers. -->
<script lang="ts">
  import { field, helpHtml, type FieldKey } from "../api/fields";

  interface Props {
    key: FieldKey;
    /** The id of the help element; the short and full texts get `-short` and `-full`. */
    id: string;
    /** The option to emphasise in the full text. */
    current?: string;
  }
  let { key, id, current }: Props = $props();

  const f = $derived(field(key));
  let more = $state(false);
</script>

<!-- eslint-disable svelte/no-at-html-tags -- helpHtml escapes everything but its own tags -->
<p class="help-short" id="{id}-short" hidden={more}>{@html helpHtml(f.short)}</p>
<p class="help-full" id="{id}-full" hidden={!more}>{@html helpHtml(f.help, current)}</p>
<!-- eslint-enable svelte/no-at-html-tags -->
<button class="link help-more" type="button" aria-expanded={more} aria-controls="{id}-full" onclick={() => (more = !more)}>
  {more ? "Less" : "More"}<span class="visually-hidden"> about {f.label}</span>
</button>
