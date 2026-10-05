<script>
  import { toasts, dismiss } from "../stores.js";
</script>

<div class="stack">
  {#each $toasts as toast (toast.id)}
    <div class="toast {toast.severity}" onclick={() => dismiss(toast.id)} role="alert">
      {toast.message}
    </div>
  {/each}
</div>

<style>
  .stack {
    position: fixed;
    right: 18px;
    bottom: 18px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    z-index: 100;
    max-width: 380px;
  }
  .toast {
    padding: 12px 16px;
    border-radius: var(--radius-sm);
    background: var(--bg-elevated);
    border: 1px solid var(--border);
    box-shadow: var(--shadow);
    color: var(--text);
    cursor: pointer;
    animation: slide 0.18s ease;
    border-left: 3px solid var(--accent);
  }
  .toast.success { border-left-color: var(--success); }
  .toast.warning { border-left-color: var(--warning); }
  .toast.error { border-left-color: var(--danger); }
  @keyframes slide {
    from { opacity: 0; transform: translateY(8px); }
    to { opacity: 1; transform: translateY(0); }
  }
</style>
