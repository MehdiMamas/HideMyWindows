<script>
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { t, locale, detectLocale } from "./lib/i18n.js";
  import { config, notify, applyTheme } from "./lib/stores.js";
  import { checkForUpdates } from "./lib/updater.js";
  import * as api from "./lib/api.js";

  import Dashboard from "./lib/pages/Dashboard.svelte";
  import QuickLaunch from "./lib/pages/QuickLaunch.svelte";
  import Rules from "./lib/pages/Rules.svelte";
  import Settings from "./lib/pages/Settings.svelte";
  import About from "./lib/pages/About.svelte";
  import Toasts from "./lib/components/Toasts.svelte";
  import CaptureStatus from "./lib/components/CaptureStatus.svelte";

  let current = $state("home");
  let ready = $state(false);

  const nav = [
    { id: "home", key: "nav.home", comp: Dashboard, icon: "M3 11l9-8 9 8v9a1 1 0 0 1-1 1h-5v-6H9v6H4a1 1 0 0 1-1-1z" },
    { id: "quick", key: "nav.quickLaunch", comp: QuickLaunch, icon: "M13 2L3 14h7l-1 8 10-12h-7z" },
    { id: "rules", key: "nav.rules", comp: Rules, icon: "M4 6h16M4 12h16M4 18h10" },
    { id: "settings", key: "nav.settings", comp: Settings, icon: "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM3 12h2m14 0h2M12 3v2m0 14v2" },
    { id: "about", key: "nav.about", comp: About, icon: "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zm0 8v6m0-9v.5" },
  ];

  const CurrentPage = $derived(nav.find((n) => n.id === current)?.comp ?? Dashboard);

  onMount(async () => {
    try {
      const cfg = await api.getConfig();
      config.set(cfg);
      applyTheme(cfg.theme);
      locale.set(detectLocale(cfg.language));
      checkForUpdates();

      // React to OS theme changes when following the system.
      window
        .matchMedia("(prefers-color-scheme: dark)")
        .addEventListener("change", () => {
          const c = $config;
          if (c && c.theme === "system") applyTheme("system");
        });

      await listen("rule-errors", (event) => {
        const errs = event.payload || [];
        if (errs.length) notify(errs[0], "warning", 6000);
      });
    } catch (e) {
      notify(String(e), "error", 0);
    } finally {
      ready = true;
    }
  });
</script>

<div class="shell">
  <aside class="sidebar">
    <div class="brand">
      <div class="logo">
        <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="2">
          <rect x="3" y="4" width="18" height="14" rx="2" />
          <path d="M3 9h18" />
        </svg>
      </div>
      <div class="brand-text">
        <strong>{$t("app.name")}</strong>
        <small>{$t("app.tagline")}</small>
      </div>
    </div>

    <nav>
      {#each nav as item}
        <button class:active={current === item.id} onclick={() => (current = item.id)}>
          <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
            <path d={item.icon} />
          </svg>
          <span>{$t(item.key)}</span>
        </button>
      {/each}
    </nav>
  </aside>

  <main>
    <header class="capture-header"><CaptureStatus /></header>
    {#if ready}
      <CurrentPage />
    {/if}
  </main>
</div>

<Toasts />

<style>
  .shell { display: flex; height: 100vh; }
  .sidebar {
    width: 230px; flex: none; background: var(--bg-elevated); border-right: 1px solid var(--border);
    display: flex; flex-direction: column; padding: 16px 12px;
  }
  .brand { display: flex; align-items: center; gap: 10px; padding: 8px 8px 18px; }
  .logo {
    width: 38px; height: 38px; border-radius: 10px; background: var(--accent); color: #fff;
    display: flex; align-items: center; justify-content: center; flex: none;
  }
  .brand-text { display: flex; flex-direction: column; overflow: hidden; }
  .brand-text strong { font-size: 15px; }
  .brand-text small { color: var(--text-faint); font-size: 11px; line-height: 1.3; }
  nav { display: flex; flex-direction: column; gap: 3px; margin-top: 6px; }
  nav button {
    display: flex; align-items: center; gap: 11px; padding: 10px 12px; border: none;
    background: transparent; color: var(--text-dim); border-radius: var(--radius-sm);
    cursor: pointer; text-align: left; font-size: 14px;
  }
  nav button:hover { background: var(--bg-hover); color: var(--text); }
  nav button.active { background: color-mix(in srgb, var(--accent) 20%, transparent); color: var(--text); }
  main {
    flex: 1; min-width: 0; padding: 26px 30px; overflow-y: auto; display: flex; flex-direction: column;
  }
  .capture-header { display: flex; justify-content: flex-end; margin-bottom: 12px; }
</style>
