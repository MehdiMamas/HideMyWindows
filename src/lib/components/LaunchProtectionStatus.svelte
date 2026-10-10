<script>
  import { onMount } from "svelte";
  import { t, locale } from "../i18n.js";
  let { status } = $props();
  let now = $state(Date.now());
  onMount(() => {
    const timer = setInterval(() => { now = Date.now(); }, 1000);
    return () => clearInterval(timer);
  });
  const remaining = $derived(Math.max(0, Math.ceil(((status?.pauseUntilMs ?? 0) - now) / 60000)));
  const phase = $derived(status?.phase === "paused" && remaining === 0 ? "starting" : (status?.phase ?? "starting"));
  const time = $derived(status?.pauseUntilMs
    ? new Date(status.pauseUntilMs).toLocaleTimeString($locale, { hour: "2-digit", minute: "2-digit" }) : "");
</script>

<div class="status" role="status">
  <strong>{$t(`launchProtection.status.${phase}`)}</strong>
  {#if phase === "paused"}
    <span>{$t("launchProtection.remaining", { minutes: remaining, time })}</span>
  {/if}
  {#if status?.error}<span class="error">{status.error}</span>{/if}
</div>

<style>
  .status { display: flex; flex-direction: column; gap: 4px; font-size: 13px; }
  .status span { color: var(--text-dim); }
  .status .error { color: var(--danger); white-space: pre-wrap; }
</style>
