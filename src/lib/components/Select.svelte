<script>
  // options: [{ value, label }]
  let { value = $bindable(), options = [], onchange } = $props();
</script>

<select
  bind:value
  onchange={(e) => onchange?.(e.currentTarget.value)}
>
  {#each options as opt}
    <option value={opt.value}>{opt.label}</option>
  {/each}
</select>

<style>
  select {
    padding: 8px 12px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: var(--bg-elevated);
    color: var(--text);
    cursor: pointer;
    min-width: 120px;
  }
  select:focus {
    outline: 2px solid var(--accent);
    outline-offset: -1px;
  }
</style>
