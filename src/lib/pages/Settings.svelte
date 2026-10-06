<script>
  import { ask } from "@tauri-apps/plugin-dialog";
  import { openPath } from "@tauri-apps/plugin-opener";
  import { t, locale, detectLocale, tr, LOCALES } from "../i18n.js";
  import { config, notify, applyTheme } from "../stores.js";
  import * as api from "../api.js";
  import Card from "../components/Card.svelte";
  import Toggle from "../components/Toggle.svelte";
  import Select from "../components/Select.svelte";
  import TextField from "../components/TextField.svelte";
  import Button from "../components/Button.svelte";

  const themeOptions = $derived([
    { value: "system", label: $t("settings.themeSystem") },
    { value: "light", label: $t("settings.themeLight") },
    { value: "dark", label: $t("settings.themeDark") },
  ]);

  const languageOptions = Object.entries(LOCALES).map(([code, l]) => ({
    value: code,
    label: l.name,
  }));

  // Numbers from <input type=number> arrive as strings; coerce before saving
  // so the Rust side (u64) can deserialize them.
  function normalize() {
    $config.ruleReapplyIntervalMs = Math.max(200, Number($config.ruleReapplyIntervalMs) || 1000);
    $config.processPollIntervalMs = Math.max(200, Number($config.processPollIntervalMs) || 1000);
  }

  function persist() {
    normalize();
    api.saveConfig($config)
      .then(() => notify($t("settings.saved"), "success"))
      .catch((e) => notify(String(e), "error"));
  }

  // Debounced persist for numeric fields.
  let timer;
  function schedulePersist() {
    clearTimeout(timer);
    timer = setTimeout(() => {
      normalize();
      api.saveConfig($config).catch((e) => notify(String(e), "error"));
    }, 500);
  }

  function onTheme(v) {
    applyTheme(v);
    persist();
  }

  function onLanguage(v) {
    locale.set(v);
    $config.language = v;
    persist();
  }

  async function openFolder() {
    try {
      const dir = await api.getConfigDir();
      await openPath(dir);
    } catch (e) {
      notify(String(e), "error");
    }
  }

  async function resetAll() {
    clearTimeout(timer);
    try {
      const confirmed = await ask(tr("settings.resetConfirm"), {
        title: tr("settings.resetConfirmTitle"),
        kind: "warning",
      });
      if (!confirmed) return;
      const next = await api.resetSettings();
      config.set(next);
      applyTheme("system");
      locale.set(detectLocale(null));
      notify(tr("settings.resetDone"), "success");
    } catch (e) {
      notify(String(e), "error");
    }
  }
</script>

<header class="page-head"><h1>{$t("settings.title")}</h1></header>

{#if $config}
  <Card title={$t("settings.appearance")}>
    <div class="field-row">
      <span>{$t("settings.theme")}</span>
      <Select bind:value={$config.theme} options={themeOptions} onchange={onTheme} />
    </div>
    <div class="field-row">
      <span>{$t("settings.language")}</span>
      <Select bind:value={$config.language} options={languageOptions} onchange={onLanguage} />
    </div>
  </Card>

  <Card title={$t("settings.behavior")}>
    <Toggle bind:checked={$config.hideSelf} label={$t("settings.hideSelf")} onchange={persist} />
    <label class="check-row">
      <input
        type="checkbox"
        checked={$config.hideNotificationToasts}
        onchange={(e) => {
          $config.hideNotificationToasts = e.currentTarget.checked;
          persist();
        }}
      />
      <span class="check-text">
        <span>{$t("settings.hideNotifications")}</span>
        <span class="check-hint">{$t("settings.hideNotificationsHint")}</span>
      </span>
    </label>
    <Toggle bind:checked={$config.closeToTray} label={$t("settings.closeToTray")} onchange={persist} />
    <Toggle bind:checked={$config.minimizeToTray} label={$t("settings.minimizeToTray")} onchange={persist} />
    <Toggle
      bind:checked={$config.startWithWindows}
      label={$t("settings.startWithWindows")}
      hint={$t("settings.startWithWindowsHint")}
      onchange={persist}
    />
  </Card>

  <Card title={$t("settings.advanced")}>
    <div class="num-row">
      <TextField type="number" bind:value={$config.ruleReapplyIntervalMs} label={$t("settings.reapplyInterval")} oninput={schedulePersist} />
      <TextField type="number" bind:value={$config.processPollIntervalMs} label={$t("settings.pollInterval")} oninput={schedulePersist} />
    </div>
    <div class="folder-row">
      <Button onclick={openFolder}>{$t("settings.openConfigFolder")}</Button>
      <Button variant="danger" onclick={resetAll}>{$t("settings.resetAll")}</Button>
    </div>
    <p class="hint">{$t("settings.restartHint")}</p>
  </Card>
{/if}

<style>
  .page-head { margin-bottom: 14px; }
  .field-row { display: flex; align-items: center; justify-content: space-between; padding: 6px 0; }
  .num-row { display: flex; gap: 14px; margin-bottom: 12px; }
  .folder-row { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 6px; }
  .hint { color: var(--text-faint); font-size: 12px; margin: 10px 0 0; }
  .check-row {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 4px 0;
    cursor: pointer;
  }
  .check-row input {
    margin-top: 3px;
    width: 16px;
    height: 16px;
    accent-color: var(--accent);
    cursor: pointer;
  }
  .check-text { display: flex; flex-direction: column; }
  .check-hint { color: var(--text-faint); font-size: 12px; }
</style>
