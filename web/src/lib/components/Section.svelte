<!-- A section that can be collapsed (DESIGN.md §10): a title and a summary
     line of the values inside, so a closed section still answers "what is
     in it?". Whether it starts open is the screen's call, per the budget. -->
<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    title: string;
    /** One line of the values inside ("4 panels · 39.9% waste"). */
    summary: string;
    open?: boolean;
    class?: string;
    children: Snippet;
  }
  let { title, summary, open = $bindable(false), class: cls = "", children }: Props = $props();
</script>

<details class="fold {cls}" bind:open>
  <summary><span class="fold-title">{title}</span><span class="fold-sum">{summary}</span></summary>
  <div class="fold-body">{@render children()}</div>
</details>
