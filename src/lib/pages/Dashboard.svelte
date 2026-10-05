<script>
  import { t } from "../i18n.js";
  import { notify } from "../stores.js";
  import * as api from "../api.js";
  import Button from "../components/Button.svelte";
  import TextField from "../components/TextField.svelte";

  let mode = $state("processes"); // "processes" | "windows"
  let processes = $state([]);
  let windows = $state([]);
  let query = $state("");
  let selected = $state(null); // { kind, pid, hwnd?, name }
  let loading = $state(false);

  async function refresh() {
    loading = true;
    try {
      if (mode === "processes") {
        processes = await api.listProcesses();
      } else {
        windows = await api.listWindows();
      }
    } catch (e) {
      notify(String(e), "error");
    } finally {
      loading = false;
    }
  }

  function switchMode(m) {
    mode = m;
    selected = null;
    refresh();
  }

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (mode === "processes") {
      return processes.filter(
        (p) => !q || p.name.toLowerCase().includes(q) || String(p.pid).includes(q),
      );
    }
    return windows.filter(
      (w) => !q || w.title.toLowerCase().includes(q) || w.class.toLowerCase().includes(q),
    );
  });

  function selectProcess(p) {
    selected = { kind: "process", pid: p.pid, name: `${p.name} (${p.pid})` };
  }
  function selectWindow(w) {
    selected = { kind: "window", hwnd: w.hwnd, pid: w.pid, name: w.title || w.class };
  }

  async function act(action, successKey) {
    if (!selected) return;
    try {
      if (selected.kind === "process") {
        await api.hideProcess(selected.pid, action);
      } else {
        await api.hideWindow(selected.hwnd, action);
      }
      notify($t(successKey, { name: selected.name }), "success");
    } catch (e) {
      notify(String(e), "error");
    }
  }

  $effect(() => {
    refresh();
  });
</script>

<header class="page-head">
  <h1>{$t("dashboard.title")}</h1>
  <p>{$t("dashboard.subtitle")}</p>
</header>

<div class="toolbar">
  <div class="tabs">
    <button class:active={mode === "processes"} onclick={() => switchMode("processes")}>
      {$t("dashboard.processes")}
    </button>
    <button class:active={mode === "windows"} onclick={() => switchMode("windows")}>
      {$t("dashboard.windows")}
    </button>
  </div>
  <div class="grow"><TextField bind:value={query} placeholder={$t("common.search")} /></div>
  <Button onclick={refresh}>{$t("common.refresh")}</Button>
</div>

<div class="list" role="listbox">
  {#if loading}
    <p class="muted">…</p>
  {:else if filtered.length === 0}
    <p class="muted">{$t("common.noResults")}</p>
  {:else if mode === "processes"}
    {#each filtered as p (p.pid)}
      <button
        class="item"
        class:sel={selected?.kind === "process" && selected.pid === p.pid}
        onclick={() => selectProcess(p)}
      >
        <span class="name">{p.name}</span>
        <span class="meta">{$t("dashboard.pid")} {p.pid}</span>
      </button>
    {/each}
  {:else}
    {#each filtered as w (w.hwnd)}
      <button
        class="item"
        class:sel={selected?.kind === "window" && selected.hwnd === w.hwnd}
        onclick={() => selectWindow(w)}
      >
        <span class="name">{w.title || "—"}</span>
        <span class="meta">{w.class} · {$t("dashboard.pid")} {w.pid}</span>
      </button>
    {/each}
  {/if}
</div>

<footer class="actions">
  <div class="sel-label">
    {selected ? $t("dashboard.selected", { name: selected.name }) : $t("common.none")}
  </div>
  <div class="buttons">
    {#if selected?.kind === "window"}
      <Button variant="primary" disabled={!selected} onclick={() => act("hideWindow", "dashboard.done")}>
        {$t("dashboard.hideWindow")}
      </Button>
      <Button disabled={!selected} onclick={() => act("unhideWindow", "dashboard.undone")}>
        {$t("dashboard.unhideWindow")}
      </Button>
    {:else}
      <Button variant="primary" disabled={!selected} onclick={() => act("hideProcessWindows", "dashboard.done")}>
        {$t("dashboard.hideProcess")}
      </Button>
      <Button disabled={!selected} onclick={() => act("unhideProcessWindows", "dashboard.undone")}>
        {$t("dashboard.unhideProcess")}
      </Button>
    {/if}
    <Button disabled={!selected} onclick={() => act("hideTrayIcon", "dashboard.done")} title={$t("dashboard.hideTray")}>
      {$t("dashboard.hideTray")}
    </Button>
  </div>
</footer>

<style>
  .page-head { margin-bottom: 14px; }
  .page-head p { color: var(--text-dim); margin: 0; }
  .toolbar { display: flex; gap: 10px; align-items: center; margin-bottom: 12px; }
  .grow { flex: 1; }
  .tabs { display: flex; background: var(--bg-elevated); border: 1px solid var(--border); border-radius: var(--radius-sm); overflow: hidden; }
  .tabs button {
    padding: 8px 14px; border: none; background: transparent; color: var(--text-dim); cursor: pointer;
  }
  .tabs button.active { background: var(--accent); color: var(--accent-contrast); }
  .list {
    flex: 1; overflow-y: auto; border: 1px solid var(--border); border-radius: var(--radius);
    background: var(--bg-card); min-height: 0;
  }
  .item {
    display: flex; justify-content: space-between; align-items: center; gap: 12px;
    width: 100%; text-align: left; padding: 10px 14px; border: none; background: transparent;
    color: var(--text); cursor: pointer; border-bottom: 1px solid var(--border);
  }
  .item:hover { background: var(--bg-hover); }
  .item.sel { background: color-mix(in srgb, var(--accent) 22%, transparent); }
  .item .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .item .meta { color: var(--text-faint); font-size: 12px; flex: none; }
  .muted { color: var(--text-faint); padding: 16px; }
  .actions { margin-top: 12px; display: flex; flex-direction: column; gap: 8px; }
  .sel-label { color: var(--text-dim); font-size: 13px; }
  .buttons { display: flex; gap: 8px; flex-wrap: wrap; }
</style>
