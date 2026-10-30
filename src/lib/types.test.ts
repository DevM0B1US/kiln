/// Shared type helpers, ported from the original `is_image_path` /
/// `QueueEntry::from_path` tests.

import { describe, expect, it } from "vitest";
import { isImagePath, pathLabel, pathParent } from "$lib/types";

describe("isImagePath", () => {
  it("accepts the known image extensions", () => {
    expect(isImagePath("/a/photo.PNG")).toBe(true);
    expect(isImagePath("/a/photo.jpeg")).toBe(true);
    expect(isImagePath("/a/photo.webp")).toBe(true);
  });

  it("rejects everything else", () => {
    expect(isImagePath("/a/notes.txt")).toBe(false);
    expect(isImagePath("/a/photo")).toBe(false);
    expect(isImagePath("/a/photo.")).toBe(false);
  });
});

describe("pathLabel", () => {
  it("keeps the file name", () => {
    expect(pathLabel("/photos/one.png")).toBe("one.png");
    expect(pathLabel("C:\\photos\\one.png")).toBe("one.png");
    expect(pathLabel("one.png")).toBe("one.png");
  });
});

describe("pathParent", () => {
  it("keeps the parent directory", () => {
    expect(pathParent("/photos/one.png")).toBe("/photos");
    expect(pathParent("C:\\photos\\one.png")).toBe("C:\\photos");
  });

  it("falls back to the current directory", () => {
    expect(pathParent("one.png")).toBe(".");
  });
});
