<script>
  import { t } from "../i18n.js";
  let { status = "unknown" } = $props();
</script>

{#if status === "hidden" || status === "partial" || status === "unknown"}
  <span class="badge" class:hidden={status === "hidden"} class:partial={status === "partial"}
    title={$t(`dashboard.captureHints.${status}`)}>
    <span class="dot" aria-hidden="true"></span>
    {$t(`dashboard.captureLabels.${status}`)}
  </span>
{/if}

<style>
  .badge {
    display: inline-flex; align-items: center; gap: 6px; padding: 3px 8px;
    flex: none; white-space: nowrap; border: 1px solid var(--border); border-radius: 999px;
    color: var(--text-dim); background: var(--bg-elevated); font-size: 11px;
  }
  .hidden { color: var(--success); }
  .partial { color: var(--warning); }
  .dot { width: 6px; height: 6px; border-radius: 50%; background: currentColor; }
</style>
