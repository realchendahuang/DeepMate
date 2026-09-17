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

  it("gives every core error code its own wording", () => {
    // Every classification the core can produce maps to a distinct key.
    // Collapsing them into one "something went wrong" line threw away the
    // diagnosis the backend had already made.
    const envelope = (code: string, message: string) => JSON.stringify({ code, message });
    expect(mapError(envelope("not_found", "harness CLI was not found on PATH"))).toBe("notFound");
    expect(mapError(envelope("timeout", "task timed out after 2m"))).toBe("timeout");
    expect(mapError(envelope("network", "connection refused"))).toBe("network");
    expect(mapError(envelope("io", "permission denied"))).toBe("io");
    expect(mapError(envelope("spawn_failed", "could not spawn dsh"))).toBe("spawnFailed");
    expect(mapError(envelope("command_failed", "exit 1"))).toBe("commandFailed");
    expect(mapError(envelope("invalid_state", "scenario X is not running"))).toBe("invalidState");
    expect(mapError(envelope("unsupported", "no opener on this platform"))).toBe("unsupported");
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
