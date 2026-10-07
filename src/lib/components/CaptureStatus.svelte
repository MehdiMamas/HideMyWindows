<script>
  import { onMount } from "svelte";
  import { config } from "../stores.js";
  import { t } from "../i18n.js";
  import { captureHidden } from "../api.js";

  let status = $state("checking");

  onMount(() => {
    let active = true;
    let request = 0;

    async function refresh() {
      const current = ++request;
      try {
        const hidden = await captureHidden();
        if (active && current === request) {
          status = hidden ? "hidden" : "visible";
        }
      } catch {
        if (active && current === request) status = "unavailable";
      }
    }

    // Includes the initial query and changes to the hide-self setting.
    const unsubscribe = config.subscribe(() => { void refresh(); });
    const timer = setInterval(refresh, 1000);
    window.addEventListener("focus", refresh);
    return () => {
      active = false;
      unsubscribe();
      clearInterval(timer);
      window.removeEventListener("focus", refresh);
    };
  });
</script>

<span class="badge" class:hidden={status === "hidden"} class:visible={status === "visible"}
  role="status" aria-live="polite" title={$t("captureStatus.hint")}>
  <span class="dot" aria-hidden="true"></span>
  {$t(`captureStatus.${status}`)}
</span>

<style>
  .badge {
    display: inline-flex; align-items: center; gap: 7px; padding: 5px 10px;
    border: 1px solid var(--border); border-radius: 999px;
    color: var(--text-dim); background: var(--bg-elevated); font-size: 12px;
  }
  .hidden { color: var(--success); }
  .visible { color: var(--warning); }
  .dot { width: 7px; height: 7px; border-radius: 50%; background: currentColor; flex: none; }
</style>
