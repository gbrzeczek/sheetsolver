<script lang="ts">
  import { mm } from "../plan/tree";
  let { kerf, trims = false }: { kerf: number; trims?: boolean } = $props();
</script>

<div class="legend">
  <span><svg width="16" height="11" aria-hidden="true" class="plan-svg"><rect x=".5" y=".5" width="15" height="10" class="p-part" /></svg>Part</span>
  <span><svg width="14" height="9" viewBox="0 0 14 8" class="plan-svg p-glyph" aria-hidden="true"><path d="M1 4h12M3.5 1.5 1 4l2.5 2.5M10.5 1.5 13 4l-2.5 2.5" fill="none" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" /></svg>Keeps grain direction</span>
  <span><svg width="12" height="12" viewBox="0 0 12 12" class="plan-svg p-glyph" aria-hidden="true"><path d="M9.6 4.3A4 4 0 1 0 10 6" fill="none" stroke-width="1.3" stroke-linecap="round" /><path d="M10.4 1.8v2.8H7.6" fill="none" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" /></svg>Turned 90°</span>
  <span><svg width="16" height="11" class="plan-svg" aria-hidden="true"><defs><pattern id="lg-h" width="4" height="4" patternUnits="userSpaceOnUse" patternTransform="rotate(45)"><rect width="4" height="4" class="p-offcut-bg" /><line x1="0" y1="0" x2="0" y2="4" class="p-hatch" /></pattern></defs><rect x=".5" y=".5" width="15" height="10" fill="url(#lg-h)" /></svg>Offcut</span>
  <span><svg width="16" height="11" aria-hidden="true" class="plan-svg"><rect x=".5" y=".5" width="15" height="10" class="p-leftover" /></svg>Leftover to keep</span>
  {#if trims}
    <span><svg width="16" height="11" class="plan-svg" aria-hidden="true"><defs><pattern id="lg-t" width="4" height="4" patternUnits="userSpaceOnUse" patternTransform="rotate(-45)"><rect width="4" height="4" class="p-trim-bg" /><line x1="0" y1="0" x2="0" y2="4" class="p-trim-hatch" /></pattern></defs><rect x=".5" y=".5" width="15" height="10" fill="url(#lg-t)" class="p-trim" /></svg>Trim, no part there</span>
  {/if}
  <span><svg width="16" height="11" class="plan-svg" aria-hidden="true"><rect x="7" y="0" width="2" height="11" class="p-kerf" /></svg>Saw cut, {mm(kerf)} mm</span>
</div>
