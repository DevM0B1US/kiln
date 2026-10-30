<script lang="ts">
  /// Calm banner under the title bar when the model file is missing.
  let { message }: { message: string } = $props();
</script>

<div class="banner">{message}</div>

<style>
  .banner {
    padding: 8px 16px;
    background: var(--surface-raised);
    border-bottom: 1px solid var(--border);
    color: var(--text-secondary);
  }
</style>
