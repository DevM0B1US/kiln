/// Small display helpers. Ports of the Rust-side formatting.

/** Bytes into a human shape: `512 B`, `1.5 MB`. */
export function formatBytes(len: number): string {
  const units = ["B", "KB", "MB", "GB"];
  if (len < 1024) {
    return `${len} B`;
  }
  let value = len;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(1)} ${units[unit]}`;
}

/** `1 image` / `3 images`, matching the Rust `image{s}` labels. */
export function pluralImages(count: number): string {
  return `${count} image${count === 1 ? "" : "s"}`;
}

/** `Export 1 image` / `Export 12 images`. */
export function exportLabel(count: number): string {
  return `Export ${count} image${count === 1 ? "" : "s"}`;
}
