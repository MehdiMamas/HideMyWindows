<script>
  let { checked = $bindable(false), label = "", hint = "", onchange } = $props();

  function toggle() {
    checked = !checked;
    onchange?.(checked);
  }
</script>

<label class="row">
  <button
    type="button"
    role="switch"
    aria-checked={checked}
    class="track"
    class:on={checked}
    onclick={toggle}
  >
    <span class="thumb"></span>
  </button>
  <span class="text">
    <span class="label">{label}</span>
    {#if hint}<span class="hint">{hint}</span>{/if}
  </span>
</label>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    cursor: pointer;
    padding: 4px 0;
  }
  .track {
    flex: none;
    width: 42px;
    height: 24px;
    border-radius: 999px;
    background: var(--border);
    border: none;
    position: relative;
    cursor: pointer;
    transition: background 0.15s;
  }
  .track.on {
    background: var(--accent);
  }
  .thumb {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: #fff;
    transition: transform 0.15s;
  }
  .track.on .thumb {
    transform: translateX(18px);
  }
  .text {
    display: flex;
    flex-direction: column;
  }
  .label {
    color: var(--text);
  }
  .hint {
    color: var(--text-faint);
    font-size: 12px;
  }
</style>
