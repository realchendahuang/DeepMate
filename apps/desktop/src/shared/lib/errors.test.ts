import { describe, expect, it } from "vitest";
import { mapError } from "./errors";

describe("mapError", () => {
  it("maps known failure patterns to friendly keys", () => {
    expect(mapError("plugin is incompatible with harness")).toBe("incompatible");
    expect(mapError("snapshot not found: foo")).toBe("notFound");
    expect(mapError("profile already exists")).toBe("alreadyExists");
    expect(mapError("network error: ECONNREFUSED")).toBe("network");
    expect(mapError("permission denied")).toBe("permission");
    expect(mapError("checksum verification failed")).toBe("checksum");
  });

  it("falls back to generic for unknown errors", () => {
    expect(mapError("some unexpected failure")).toBe("generic");
    expect(mapError(new Error("boom"))).toBe("generic");
  });

  it("handles non-string, non-Error values", () => {
    expect(mapError(42)).toBe("generic");
    expect(mapError(null)).toBe("generic");
  });
});
