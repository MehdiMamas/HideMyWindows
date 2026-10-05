<script>
  let { title = "", children } = $props();
</script>

<section class="card">
  {#if title}<h3>{title}</h3>{/if}
  {@render children?.()}
</section>

<style>
  .card {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 18px 20px;
    margin-bottom: 16px;
  }
  .card h3 {
    margin-bottom: 14px;
    font-size: 15px;
    color: var(--text-dim);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    font-size: 12px;
  }
</style>
