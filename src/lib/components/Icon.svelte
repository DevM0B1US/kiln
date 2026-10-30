<script lang="ts">
  /// One inline Lucide mark. The `body` is a compile-time constant from
  /// `$lib/icons` — never user input — so the single {@html} here is safe.
  let { body, size = 14 }: { body: string; size?: number } = $props();
</script>

<!-- eslint-disable-next-line svelte/no-at-html-tags -->
{@html `<svg class="icon" viewBox="0 0 24 24" style="width:${size}px;height:${size}px">${body}</svg>`}
