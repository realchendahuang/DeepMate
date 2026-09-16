import { describe, expect, it } from "vitest";
import { errorMessage, mapError } from "./errors";

describe("mapError", () => {
  it("maps known failure patterns to friendly keys", () => {
    expect(mapError("plugin is incompatible with harness")).toBe("incompatible");
    expect(mapError("snapshot not found: foo")).toBe("notFound");
    expect(mapError("profile already exists")).toBe("alreadyExists");
    expect(mapError("network error: ECONNREFUSED")).toBe("network");
    expect(mapError("permission denied")).toBe("permission");
    expect(mapError("checksum verification failed")).toBe("checksum");
  });

  it("maps the JSON envelope codes from the Rust side exactly", () => {
    const envelope = (code: string, message: string) => JSON.stringify({ code, message });
    expect(mapError(envelope("not_found", "harness CLI was not found on PATH"))).toBe("notFound");
    expect(mapError(envelope("timeout", "task timed out after 2m"))).toBe("timeout");
    expect(mapError(envelope("invalid_state", "scenario X is not running"))).toBe("generic");
    // A code without a mapping falls through to pattern matching.
    expect(mapError(envelope("mystery", "profile already exists"))).toBe("alreadyExists");
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

describe("errorMessage", () => {
  it("unwraps the JSON envelope message", () => {
    expect(
      errorMessage(JSON.stringify({ code: "not_found", message: "harness CLI was not found" })),
    ).toBe("harness CLI was not found");
  });

  it("keeps plain strings and Error messages as they are", () => {
    expect(errorMessage("plain failure")).toBe("plain failure");
    expect(errorMessage(new Error("boom"))).toBe("boom");
  });
});
