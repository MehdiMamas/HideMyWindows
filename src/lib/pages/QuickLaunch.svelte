<script module>
  // The app list takes seconds to build; keep it while the app runs.
  const cache = { apps: null, icons: {} };

  /** Forget the cached app list (tests). */
  export function clearAppCache() {
    cache.apps = null;
    cache.icons = {};
  }
</script>

<script>
  import { open } from "@tauri-apps/plugin-dialog";
  import { t } from "../i18n.js";
  import { config, notify } from "../stores.js";
  import * as api from "../api.js";
  import Button from "../components/Button.svelte";
  import TextField from "../components/TextField.svelte";
  import Toggle from "../components/Toggle.svelte";

  let editing = $state(null); // entry being edited (draft)
  let picking = $state(false);
  let apps = $state(cache.apps); // installed apps, null while loading
  let icons = $state(cache.icons); // app key -> data URL
  let query = $state("");
  let addToStart = $state(true);
  let busy = $state(false);

  // What an app launches. Start can list one exe several times with
  // different arguments (Java, EA, Command Prompt), so arguments count.
  const appKey = (app) => app.appId || `${app.path.toLowerCase()} ${app.arguments || ""}`;
  // One picker row; the backend drops exact duplicates, so this is unique.
  const rowKey = (app) => [app.name, app.appId, app.path, app.arguments].join("|");

  let matches = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return (apps || []).filter((app) => !q || app.name.toLowerCase().includes(q));
  });

  function persist() {
    api.saveConfig($config).catch((e) => notify(String(e), "error"));
  }

  function store(entry) {
    config.update((c) => {
      const exists = c.quickLaunch.some((e) => e.id === entry.id);
      const list = exists
        ? c.quickLaunch.map((e) => (e.id === entry.id ? entry : e))
        : [...c.quickLaunch, entry];
      return { ...c, quickLaunch: list };
    });
    persist();
  }

  async function loadApps() {
    try {
      // Names first; icons take a few seconds longer.
      if (!apps) apps = cache.apps = await api.listInstalledApps(false);
      if (!Object.keys(icons).length) {
        const withIcons = await api.listInstalledApps(true);
        icons = cache.icons = Object.fromEntries(
          withIcons.filter((a) => a.icon).map((a) => [appKey(a), a.icon]),
        );
      }
    } catch (e) {
      apps ??= [];
      notify(String(e), "error");
    }
  }

  function openPicker() {
    query = "";
    picking = true;
    loadApps();
  }

  async function pick(app) {
    if (busy) return;
    const existing = $config.quickLaunch.find((e) => appKey(e) === appKey(app));
    if (existing) {
      picking = false;
      notify($t("quickLaunch.alreadyAdded", { name: displayName(existing) }), "info");
      return;
    }
    busy = true;
    const entry = {
      id: crypto.randomUUID(),
      name: app.name,
      path: app.path,
      arguments: app.arguments || "",
      appId: app.appId,
      shortcut: "",
    };
    if (addToStart) entry.shortcut = await createShortcut(entry);
    store(entry);
    busy = false;
    picking = false;
    notify(
      entry.shortcut
        ? $t("quickLaunch.startMenuHint", { shortcut: shortcutName(entry) })
        : $t("quickLaunch.added", { name: entry.name }),
      "success",
      entry.shortcut ? 9000 : 4000,
    );
  }

  function newExeEntry() {
    picking = false;
    editing = { id: crypto.randomUUID(), name: "", path: "", arguments: "", appId: "", shortcut: "", _new: true };
  }

  function edit(entry) {
    editing = { ...entry };
  }

  async function browse() {
    const file = await open({
      multiple: false,
      filters: [{ name: "Executable", extensions: ["exe"] }],
    });
    if (file) editing.path = typeof file === "string" ? file : file.path;
  }

  async function saveEntry() {
    if (!editing.appId && !editing.path.trim()) {
      notify($t("quickLaunch.path"), "warning");
      return;
    }
    const entry = {
      id: editing.id,
      name: editing.name,
      path: editing.path,
      arguments: editing.arguments,
      appId: editing.appId || "",
      shortcut: editing.shortcut || "",
    };
    // Keep an existing Start menu shortcut in step with the edited entry.
    if (entry.shortcut) {
      api.removeLaunchShortcut(entry.shortcut).catch(() => {});
      entry.shortcut = await createShortcut(entry);
    }
    store(entry);
    editing = null;
  }

  function remove(entry) {
    if (entry.shortcut) api.removeLaunchShortcut(entry.shortcut).catch(() => {});
    config.update((c) => ({
      ...c,
      quickLaunch: c.quickLaunch.filter((e) => e.id !== entry.id),
    }));
    persist();
  }

  async function createShortcut(entry) {
    try {
      return await api.createLaunchShortcut(displayName(entry), entry.path, entry.arguments || "", entry.appId);
    } catch (e) {
      notify(String(e), "error");
      return "";
    }
  }

  async function toggleShortcut(entry) {
    if (entry.shortcut) {
      api.removeLaunchShortcut(entry.shortcut).catch(() => {});
      store({ ...entry, shortcut: "" });
      return;
    }
    const shortcut = await createShortcut(entry);
    if (shortcut) {
      store({ ...entry, shortcut });
      notify($t("quickLaunch.startMenuHint", { shortcut: shortcutName(entry) }), "success", 9000);
    }
  }

  async function launch(entry) {
    try {
      await api.quickLaunch(entry.path, entry.arguments || "", entry.appId);
      notify($t("quickLaunch.launched", { name: displayName(entry) }), "success");
    } catch (e) {
      notify(String(e), "error");
    }
  }

  function fileName(path) {
    return path.split(/[\\/]/).pop();
  }

  function displayName(entry) {
    return entry.name || fileName(entry.path) || entry.appId;
  }

  function shortcutName(entry) {
    return `${displayName(entry)} HideMyWindows`;
  }

  function entryIcon(entry) {
    return icons[appKey(entry)];
  }

  $effect(() => {
    // Card icons come from the same list; load it once quietly.
    if ($config?.quickLaunch?.length && apps === null) loadApps();
  });
</script>

<header class="page-head">
  <h1>{$t("quickLaunch.title")}</h1>
  <p>{$t("quickLaunch.subtitle")}</p>
</header>

<div class="bar">
  <Button variant="primary" onclick={openPicker}>+ {$t("quickLaunch.add")}</Button>
</div>

{#if $config?.quickLaunch?.length}
  <div class="grid">
    {#each $config.quickLaunch as entry (entry.id)}
      <div class="entry">
        <button class="launch" onclick={() => launch(entry)} title={$t("quickLaunch.launch")}>
          {#if entryIcon(entry)}
            <img class="icon" src={entryIcon(entry)} alt="" />
          {:else}
            <span class="icon placeholder">{displayName(entry).slice(0, 1).toUpperCase()}</span>
          {/if}
          <span class="text">
            <span class="title">{displayName(entry)}</span>
            <span class="path">{entry.appId ? $t("quickLaunch.storeApp") : entry.path}</span>
          </span>
        </button>
        <div class="entry-actions">
          <Button variant="ghost" onclick={() => toggleShortcut(entry)}>
            {entry.shortcut ? $t("quickLaunch.removeShortcut") : $t("quickLaunch.addShortcut")}
          </Button>
          <span class="spacer"></span>
          <Button variant="ghost" onclick={() => edit(entry)}>{$t("common.edit")}</Button>
          <Button variant="ghost" onclick={() => remove(entry)}>{$t("common.remove")}</Button>
        </div>
      </div>
    {/each}
  </div>
{:else}
  <p class="muted">{$t("quickLaunch.empty")}</p>
{/if}

{#if picking}
  <div class="overlay" onclick={() => (picking = false)}>
    <div class="dialog picker" onclick={(e) => e.stopPropagation()}>
      <h3>{$t("quickLaunch.pickTitle")}</h3>
      <TextField bind:value={query} placeholder={$t("quickLaunch.searchApps")} />
      <div class="app-list" aria-busy={apps === null}>
        {#if apps === null}
          <p class="muted">{$t("quickLaunch.loadingApps")}</p>
        {:else if !matches.length}
          <p class="muted">{$t("common.noResults")}</p>
        {:else}
          {#each matches as app (rowKey(app))}
            <button class="app" disabled={busy} onclick={() => pick(app)}>
              {#if icons[appKey(app)]}
                <img class="icon" src={icons[appKey(app)]} alt="" />
              {:else}
                <span class="icon placeholder">{app.name.slice(0, 1).toUpperCase()}</span>
              {/if}
              <span class="text">
                <span class="title">{app.name}</span>
                <span class="path">{app.appId ? $t("quickLaunch.storeApp") : fileName(app.path)}</span>
              </span>
            </button>
          {/each}
        {/if}
      </div>
      <Toggle bind:checked={addToStart} label={$t("quickLaunch.addToStart")} hint={$t("quickLaunch.addToStartHint")} />
      <div class="dialog-actions">
        <button class="link" onclick={newExeEntry}>{$t("quickLaunch.addExe")}</button>
        <span class="spacer"></span>
        <Button variant="ghost" onclick={() => (picking = false)}>{$t("common.cancel")}</Button>
      </div>
    </div>
  </div>
{/if}

{#if editing}
  <div class="overlay" onclick={() => (editing = null)}>
    <div class="dialog" onclick={(e) => e.stopPropagation()}>
      <h3>{editing._new ? $t("quickLaunch.addExeTitle") : $t("quickLaunch.edit")}</h3>
      <TextField bind:value={editing.name} label={$t("quickLaunch.name")} />
      {#if !editing.appId}
        <div class="path-row">
          <TextField bind:value={editing.path} label={$t("quickLaunch.path")} />
          <Button onclick={browse}>{$t("common.browse")}</Button>
        </div>
      {/if}
      <TextField bind:value={editing.arguments} label={$t("quickLaunch.arguments")} />
      <div class="dialog-actions">
        <span class="spacer"></span>
        <Button variant="ghost" onclick={() => (editing = null)}>{$t("common.cancel")}</Button>
        <Button variant="primary" onclick={saveEntry}>{$t("common.save")}</Button>
      </div>
    </div>
  </div>
{/if}

<style>
  .page-head { margin-bottom: 14px; }
  .page-head p { color: var(--text-dim); margin: 0; }
  .bar { margin-bottom: 14px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 12px; }
  .entry {
    background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius);
    display: flex; flex-direction: column;
  }
  .launch, .app {
    text-align: left; border: none; background: transparent; color: var(--text);
    padding: 12px 14px; cursor: pointer; display: flex; align-items: center; gap: 12px; border-radius: var(--radius);
  }
  .launch:hover, .app:hover:not(:disabled) { background: var(--bg-hover); }
  .app:disabled { cursor: progress; opacity: 0.6; }
  .icon { width: 32px; height: 32px; flex: none; }
  .placeholder {
    display: flex; align-items: center; justify-content: center; border-radius: 8px;
    background: var(--bg-hover); color: var(--text-dim); font-weight: 600;
  }
  .text { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .title { font-weight: 600; }
  .path { color: var(--text-faint); font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .entry-actions { display: flex; align-items: center; gap: 4px; padding: 6px 10px; border-top: 1px solid var(--border); }
  .spacer { flex: 1; }
  .muted { color: var(--text-faint); }
  .overlay {
    position: fixed; inset: 0; background: rgba(0,0,0,0.5); display: flex; align-items: center; justify-content: center; z-index: 50;
  }
  .dialog {
    background: var(--bg-elevated); border: 1px solid var(--border); border-radius: var(--radius);
    padding: 22px; width: min(460px, 90vw); box-shadow: var(--shadow); display: flex; flex-direction: column; gap: 14px;
  }
  .picker { width: min(560px, 92vw); max-height: 86vh; }
  .app-list {
    display: flex; flex-direction: column; overflow-y: auto; min-height: 120px; max-height: 50vh;
    border: 1px solid var(--border); border-radius: var(--radius-sm); padding: 4px;
  }
  .app { padding: 8px 10px; }
  .app .icon { width: 28px; height: 28px; }
  .path-row { display: flex; gap: 8px; align-items: flex-end; }
  .dialog-actions { display: flex; align-items: center; gap: 8px; margin-top: 4px; }
  .link {
    border: none; background: none; color: var(--accent); cursor: pointer; padding: 0; font: inherit; font-size: 13px;
  }
  .link:hover { text-decoration: underline; }
</style>
