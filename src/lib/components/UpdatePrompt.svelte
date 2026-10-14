<script>
  import { t } from "../i18n.js";
  import { updatePrompt } from "../updater.js";
  import Button from "./Button.svelte";

  let laterButton = $state();

  // "Later" is the safe choice: it has focus, so Enter and Escape both decline.
  $effect(() => {
    if ($updatePrompt) laterButton?.querySelector("button")?.focus();
  });

  function onkeydown(event) {
    if ($updatePrompt && event.key === "Escape") $updatePrompt.answer(false);
  }
</script>

<svelte:window {onkeydown} />

{#if $updatePrompt}
  <div class="backdrop">
    <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="update-title">
      <h2 id="update-title">{$t("about.updateTitle")}</h2>
      <p>{$t("about.updateIntro", { version: $updatePrompt.version })}</p>
      <p class="what">{$t("about.updateWhat")}</p>
      {#if $updatePrompt.notes}
        <h3>{$t("about.updateNotes")}</h3>
        <div class="notes">{$updatePrompt.notes}</div>
      {/if}
      <div class="actions">
        <Button variant="primary" onclick={() => $updatePrompt.answer(true)}>
          {$t("about.updateInstall")}
        </Button>
        <span bind:this={laterButton}>
          <Button onclick={() => $updatePrompt.answer(false)}>{$t("about.updateLater")}</Button>
        </span>
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 200;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, #000 45%, transparent);
  }
  .dialog {
    width: min(560px, calc(100vw - 48px));
    max-height: calc(100vh - 48px);
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 22px;
    border-radius: var(--radius-sm);
    background: var(--bg-elevated);
    border: 1px solid var(--border);
    box-shadow: var(--shadow);
    color: var(--text);
  }
  h2 { margin: 0; font-size: 18px; }
  h3 { margin: 6px 0 0; font-size: 14px; }
  p { margin: 0; }
  .what { font-weight: 600; }
  .notes {
    overflow: auto;
    min-height: 60px;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    white-space: pre-wrap;
    font-size: 13px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 6px;
  }
</style>
