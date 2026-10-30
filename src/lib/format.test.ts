/// Display formatting, ported from the original `format_bytes`.

import { describe, expect, it } from "vitest";
import { exportLabel, formatBytes, pluralImages } from "$lib/format";

describe("formatBytes", () => {
  it("keeps small values in bytes", () => {
    expect(formatBytes(512)).toBe("512 B");
  });

  it("climbs the units with one decimal", () => {
    expect(formatBytes(2048)).toBe("2.0 KB");
    expect(formatBytes(1536 * 1024)).toBe("1.5 MB");
  });
});

describe("plurals", () => {
  it("singular and plural", () => {
    expect(pluralImages(1)).toBe("1 image");
    expect(pluralImages(3)).toBe("3 images");
    expect(exportLabel(1)).toBe("Export 1 image");
    expect(exportLabel(12)).toBe("Export 12 images");
  });
});
