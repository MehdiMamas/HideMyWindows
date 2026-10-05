<script>
  import { open } from "@tauri-apps/plugin-dialog";
  import { t } from "../i18n.js";
  import { config, notify } from "../stores.js";
  import * as api from "../api.js";
  import Button from "../components/Button.svelte";
  import TextField from "../components/TextField.svelte";

  let editing = $state(null); // entry being edited (draft)

  function persist() {
    api.saveConfig($config).catch((e) => notify(String(e), "error"));
  }

  function newEntry() {
    editing = { id: crypto.randomUUID(), name: "", path: "", arguments: "", _new: true };
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

  function saveEntry() {
    if (!editing.path.trim()) {
      notify($t("quickLaunch.path"), "warning");
      return;
    }
    const entry = {
      id: editing.id,
      name: editing.name,
      path: editing.path,
      arguments: editing.arguments,
    };
    config.update((c) => {
      const list = c.quickLaunch.filter((e) => e.id !== entry.id);
      list.push(entry);
      return { ...c, quickLaunch: list };
    });
    persist();
    editing = null;
  }

  function remove(entry) {
    config.update((c) => ({
      ...c,
      quickLaunch: c.quickLaunch.filter((e) => e.id !== entry.id),
    }));
    persist();
  }

  async function launch(entry) {
    try {
      await api.quickLaunch(entry.path, entry.arguments || "");
      notify($t("quickLaunch.launched", { name: entry.name || entry.path }), "success");
    } catch (e) {
      notify(String(e), "error");
    }
  }

  function fileName(path) {
    return path.split(/[\\/]/).pop();
  }
</script>

<header class="page-head">
  <h1>{$t("quickLaunch.title")}</h1>
  <p>{$t("quickLaunch.subtitle")}</p>
</header>

<div class="bar">
  <Button variant="primary" onclick={newEntry}>+ {$t("quickLaunch.add")}</Button>
</div>

{#if $config?.quickLaunch?.length}
  <div class="grid">
    {#each $config.quickLaunch as entry (entry.id)}
      <div class="entry">
        <button class="launch" onclick={() => launch(entry)} title={$t("quickLaunch.launch")}>
          <span class="title">{entry.name || fileName(entry.path)}</span>
          <span class="path">{entry.path}</span>
        </button>
        <div class="entry-actions">
          <Button variant="ghost" onclick={() => edit(entry)}>{$t("common.edit")}</Button>
          <Button variant="ghost" onclick={() => remove(entry)}>{$t("common.remove")}</Button>
        </div>
      </div>
    {/each}
  </div>
{:else}
  <p class="muted">{$t("quickLaunch.empty")}</p>
{/if}

{#if editing}
  <div class="overlay" onclick={() => (editing = null)}>
    <div class="dialog" onclick={(e) => e.stopPropagation()}>
      <h3>{editing._new ? $t("quickLaunch.add") : $t("quickLaunch.edit")}</h3>
      <TextField bind:value={editing.name} label={$t("quickLaunch.name")} />
      <div class="path-row">
        <TextField bind:value={editing.path} label={$t("quickLaunch.path")} />
        <Button onclick={browse}>{$t("common.browse")}</Button>
      </div>
      <TextField bind:value={editing.arguments} label={$t("quickLaunch.arguments")} />
      <div class="dialog-actions">
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
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 12px; }
  .entry {
    background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius);
    display: flex; flex-direction: column;
  }
  .launch {
    text-align: left; border: none; background: transparent; color: var(--text);
    padding: 14px 16px; cursor: pointer; display: flex; flex-direction: column; gap: 4px; border-radius: var(--radius);
  }
  .launch:hover { background: var(--bg-hover); }
  .launch .title { font-weight: 600; }
  .launch .path { color: var(--text-faint); font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .entry-actions { display: flex; justify-content: flex-end; gap: 4px; padding: 6px 10px; border-top: 1px solid var(--border); }
  .muted { color: var(--text-faint); }
  .overlay {
    position: fixed; inset: 0; background: rgba(0,0,0,0.5); display: flex; align-items: center; justify-content: center; z-index: 50;
  }
  .dialog {
    background: var(--bg-elevated); border: 1px solid var(--border); border-radius: var(--radius);
    padding: 22px; width: min(460px, 90vw); box-shadow: var(--shadow); display: flex; flex-direction: column; gap: 14px;
  }
  .path-row { display: flex; gap: 8px; align-items: flex-end; }
  .dialog-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 4px; }
</style>
