import { describe, expect, it } from "vitest";
import { isBlocked } from "./store";

describe("isBlocked", () => {
  it("never blocks when nothing is running", () => {
    expect(isBlocked(null, "install")).toBe(false);
    expect(isBlocked(null, "load")).toBe(false);
  });

  it("blocks the same action while it is running", () => {
    expect(isBlocked("install", "install")).toBe(true);
    expect(isBlocked("load", "load")).toBe(true);
  });

  it("lets read-only operations run during any other operation", () => {
    expect(isBlocked("install", "load")).toBe(false);
    expect(isBlocked("install", "search")).toBe(false);
    expect(isBlocked("install", "doctor")).toBe(false);
    expect(isBlocked("refresh", "load")).toBe(false);
  });

  it("blocks mutating operations while another mutation runs", () => {
    expect(isBlocked("install", "remove")).toBe(true);
    expect(isBlocked("remove", "update")).toBe(true);
    expect(isBlocked("save", "snapshot")).toBe(true);
    expect(isBlocked("update-install", "config")).toBe(true);
  });

  it("does not block mutations during read-only operations", () => {
    expect(isBlocked("load", "install")).toBe(false);
    expect(isBlocked("search", "remove")).toBe(false);
    expect(isBlocked("doctor", "save")).toBe(false);
  });
});
