<script>
  import { t } from "../i18n.js";
  import { config, notify } from "../stores.js";
  import { ruleStatus } from "../ruleStatus.js";
  import * as api from "../api.js";
  import Button from "../components/Button.svelte";
  import Select from "../components/Select.svelte";
  import TextField from "../components/TextField.svelte";
  import Toggle from "../components/Toggle.svelte";

  const targetOptions = $derived([
    { value: "windowTitle", label: $t("rules.targets.windowTitle") },
    { value: "windowClass", label: $t("rules.targets.windowClass") },
    { value: "processName", label: $t("rules.targets.processName") },
    { value: "processId", label: $t("rules.targets.processId") },
  ]);
  const comparatorOptions = $derived([
    { value: "equals", label: $t("rules.comparators.equals") },
    { value: "contains", label: $t("rules.comparators.contains") },
    { value: "startsWith", label: $t("rules.comparators.startsWith") },
    { value: "endsWith", label: $t("rules.comparators.endsWith") },
    { value: "regex", label: $t("rules.comparators.regex") },
  ]);
  const actionOptions = $derived([
    { value: "hideProcessWindows", label: $t("rules.actions.hideProcessWindows") },
    { value: "hideWindow", label: $t("rules.actions.hideWindow") },
    { value: "unhideProcessWindows", label: $t("rules.actions.unhideProcessWindows") },
    { value: "unhideWindow", label: $t("rules.actions.unhideWindow") },
    { value: "hideTrayIcon", label: $t("rules.actions.hideTrayIcon") },
    { value: "unhideTrayIcon", label: $t("rules.actions.unhideTrayIcon") },
  ]);

  function persist() {
    api.saveConfig($config).catch((e) => notify(String(e), "error"));
  }

  function addRule() {
    config.update((c) => ({
      ...c,
      windowRules: [
        ...c.windowRules,
        {
          id: crypto.randomUUID(),
          target: "processName",
          comparator: "equals",
          value: "",
          action: "hideProcessWindows",
          enabled: false,
          persistent: false,
        },
      ],
    }));
  }

  function removeRule(id) {
    config.update((c) => ({ ...c, windowRules: c.windowRules.filter((r) => r.id !== id) }));
    persist();
  }

  // Debounce only the free-text value so a half-typed name is not applied.
  // Selects, toggles, and remove save immediately; the watcher then reconciles.
  let timer;
  function schedulePersist() {
    clearTimeout(timer);
    timer = setTimeout(persist, 400);
  }
</script>

<header class="page-head">
  <h1>{$t("rules.title")}</h1>
  <p>{$t("rules.subtitle")}</p>
</header>

{#if $ruleStatus}
  <section class="rule-result" class:has-issues={$ruleStatus.errors.length > 0} aria-label={$t("rules.resultTitle")}>
    <strong>{$t("rules.resultTitle")}</strong>
    <p aria-live="polite">{$t("rules.resultCounts", { hidden: $ruleStatus.hiddenWindows, matched: $ruleStatus.matchedWindows })}</p>
    {#if $ruleStatus.unknownWindows}
      <p>{$t("rules.resultUnknown", { count: $ruleStatus.unknownWindows })}</p>
    {/if}
    {#if $ruleStatus.errors.length}
      <details>
        <summary>{$t("rules.resultIssues", { count: $ruleStatus.errors.length })}</summary>
        <ul>{#each $ruleStatus.errors as error}<li>{error}</li>{/each}</ul>
      </details>
    {/if}
  </section>
{/if}

<div class="bar">
  <Button variant="primary" onclick={addRule}>+ {$t("rules.add")}</Button>
</div>

{#if $config?.windowRules?.length}
  <div class="rules">
    {#each $config.windowRules as rule (rule.id)}
      <div class="rule">
        <div class="line">
          <Select bind:value={rule.target} options={targetOptions} onchange={persist} />
          <Select bind:value={rule.comparator} options={comparatorOptions} onchange={persist} />
          <div class="val"><TextField bind:value={rule.value} placeholder={$t("rules.value")} oninput={schedulePersist} /></div>
        </div>
        <div class="line">
          <Select bind:value={rule.action} options={actionOptions} onchange={persist} />
          <Toggle bind:checked={rule.enabled} label={$t("rules.enabled")} onchange={persist} />
          <Toggle bind:checked={rule.persistent} label={$t("rules.persistent")} onchange={persist} />
          <div class="spacer"></div>
          <Button variant="danger" onclick={() => removeRule(rule.id)}>{$t("common.remove")}</Button>
        </div>
      </div>
    {/each}
  </div>
{:else}
  <p class="muted">{$t("rules.empty")}</p>
{/if}

<style>
  .page-head { margin-bottom: 14px; }
  .page-head p { color: var(--text-dim); margin: 0; }
  .bar { margin-bottom: 14px; }
  .rule-result { background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius); padding: 14px 16px; margin-bottom: 14px; }
  .rule-result.has-issues { border-left: 3px solid var(--warning); }
  .rule-result p { margin: 6px 0; color: var(--text-dim); }
  .rule-result summary { cursor: pointer; color: var(--warning); }
  .rule-result ul { padding-left: 20px; margin-bottom: 0; }
  .rule-result li { overflow-wrap: anywhere; margin-top: 6px; }
  .rules { display: flex; flex-direction: column; gap: 12px; }
  .rule {
    background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius);
    padding: 14px 16px; display: flex; flex-direction: column; gap: 12px;
  }
  .line { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; }
  .val { flex: 1; min-width: 160px; }
  .spacer { flex: 1; }
  .muted { color: var(--text-faint); }
</style>
