<script>
  let {
    variant = "default",
    disabled = false,
    title = "",
    onclick,
    children,
  } = $props();
</script>

<button class="btn {variant}" {disabled} {title} {onclick}>
  {@render children?.()}
</button>

<style>
  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 8px 14px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: var(--bg-elevated);
    color: var(--text);
    cursor: pointer;
    transition: background 0.12s, border-color 0.12s, opacity 0.12s;
    white-space: nowrap;
  }
  .btn:hover:not(:disabled) {
    background: var(--bg-hover);
  }
  .btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  .primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-contrast);
  }
  .primary:hover:not(:disabled) {
    background: var(--accent-hover);
    border-color: var(--accent-hover);
  }
  .danger {
    color: var(--danger);
    border-color: color-mix(in srgb, var(--danger) 45%, transparent);
  }
  .danger:hover:not(:disabled) {
    background: color-mix(in srgb, var(--danger) 14%, transparent);
  }
  .ghost {
    background: transparent;
    border-color: transparent;
  }
  .ghost:hover:not(:disabled) {
    background: var(--bg-hover);
  }
</style>
