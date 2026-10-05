<script>
  let {
    value = $bindable(""),
    placeholder = "",
    type = "text",
    label = "",
    oninput,
  } = $props();
</script>

<label class="field">
  {#if label}<span class="label">{label}</span>{/if}
  <input
    {type}
    {placeholder}
    bind:value
    oninput={(e) => oninput?.(e.currentTarget.value)}
  />
</label>

<style>
  .field {
    display: flex;
    flex-direction: column;
    gap: 5px;
    width: 100%;
  }
  .label {
    color: var(--text-dim);
    font-size: 12.5px;
  }
  input {
    padding: 9px 12px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: var(--bg-elevated);
    color: var(--text);
    width: 100%;
  }
  input:focus {
    outline: 2px solid var(--accent);
    outline-offset: -1px;
  }
</style>
