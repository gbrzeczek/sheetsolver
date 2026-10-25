<!-- A segmented row of options: a radio group (arrow keys work natively). -->
<script lang="ts">
  import { options, type FieldKey } from "../api/fields";

  interface Props {
    key: FieldKey;
    value: string | number;
    name: string;
    describedby?: string;
    disabled?: boolean;
    onchange(value: string | number): void;
  }
  let { key, value, name, describedby, disabled = false, onchange }: Props = $props();
</script>

<div class="radio-row" role="radiogroup" aria-describedby={describedby}>
  {#each options(key) as option (option.value)}
    <label class:on={option.value === value} class:disabled>
      <input
        type="radio"
        {name}
        value={option.value}
        checked={option.value === value}
        aria-describedby={describedby}
        {disabled}
        onchange={() => onchange(option.value)}
      />{option.title}
    </label>
  {/each}
</div>
