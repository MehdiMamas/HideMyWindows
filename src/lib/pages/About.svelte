<script>
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { t } from "../i18n.js";
  import * as api from "../api.js";
  import { checkForUpdates, updateStatus } from "../updater.js";
  import Card from "../components/Card.svelte";
  import Button from "../components/Button.svelte";

  const REPO = "https://github.com/mehdimamas/hidemywindows";
  let version = $state("2.0.0");

  const downloadedLabel = $derived.by(() => {
    const { downloaded, total } = $updateStatus;
    if (!total) return formatBytes(downloaded);
    const pct = Math.min(100, Math.round((downloaded / total) * 100));
    return `${pct}% · ${formatBytes(downloaded)} / ${formatBytes(total)}`;
  });

  function formatBytes(n) {
    if (!n) return "0 MB";
    return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  }

  function onCheck() {
    checkForUpdates({ interactive: true });
  }

  $effect(() => {
    api.appVersion().then((v) => (version = v)).catch(() => {});
  });

  function link(url) {
    openUrl(url).catch(() => {});
  }
</script>

<header class="page-head">
  <h1>{$t("about.title")}</h1>
  <p class="ver">{$t("about.version", { version })}</p>
  <div class="update">
    <Button onclick={onCheck} disabled={$updateStatus.phase !== "idle"}>
      {$updateStatus.phase === "checking" ? $t("about.checking") : $t("about.checkForUpdates")}
    </Button>
    {#if $updateStatus.phase === "downloading"}
      <p class="dl">{$t("about.downloading", { version: $updateStatus.version })}</p>
      {#if $updateStatus.total}
        <progress max={$updateStatus.total} value={$updateStatus.downloaded}></progress>
      {:else}
        <progress></progress>
      {/if}
      <span class="dl-meta">{downloadedLabel}</span>
    {/if}
  </div>
</header>

<Card>
  <p>{$t("about.what")}</p>
  <p class="cont">{$t("about.continuation")}</p>
</Card>

<Card title={$t("about.credits")}>
  <ul class="credits">
    <li>{$t("about.originalAuthor")}</li>
    <li>{$t("about.originalHelp")}</li>
    <li>{$t("about.maintainer")}</li>
  </ul>
  <p class="license">{$t("about.license")}</p>
</Card>

<div class="links">
  <Button onclick={() => link(REPO)}>{$t("about.sourceCode")}</Button>
  <Button onclick={() => link(REPO + "/issues")}>{$t("about.reportIssue")}</Button>
</div>

<style>
  .page-head { margin-bottom: 14px; }
  .ver { color: var(--text-faint); margin: 0; }
  .update { display: flex; flex-direction: column; align-items: flex-start; gap: 8px; margin-top: 12px; }
  .dl { margin: 0; font-size: 13px; }
  .dl-meta { color: var(--text-faint); font-size: 12px; }
  progress { width: min(320px, 100%); height: 8px; }
  .cont { color: var(--text-dim); border-left: 3px solid var(--accent); padding-left: 12px; }
  .credits { margin: 0; padding-left: 18px; line-height: 1.9; }
  .license { color: var(--text-faint); font-size: 13px; margin-bottom: 0; }
  .links { display: flex; gap: 10px; }
</style>
