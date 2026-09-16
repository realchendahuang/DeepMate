import { describe, expect, it } from "vitest";
import { isMutating, runBusy, useBusyStore } from "./busy";

// A deferred promise the tests can resolve/reject on demand.
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe("isMutating", () => {
  it("classifies mutating actions", () => {
    expect(isMutating("install")).toBe(true);
    expect(isMutating("remove")).toBe(true);
    expect(isMutating("save")).toBe(true);
    expect(isMutating("doctor-fix")).toBe(true);
  });

  it("classifies read-only actions", () => {
    expect(isMutating("load")).toBe(false);
    expect(isMutating("refresh")).toBe(false);
    expect(isMutating("search")).toBe(false);
    expect(isMutating("doctor")).toBe(false);
    expect(isMutating("runtime")).toBe(false);
  });
});

describe("runBusy", () => {
  it("tracks the action while it runs and clears it after", async () => {
    const gate = deferred<void>();
    const running = runBusy("install", () => gate.promise);
    expect(useBusyStore.getState().active).toContain("install");
    gate.resolve();
    await running;
    expect(useBusyStore.getState().active).toEqual([]);
  });

  it("keeps a concurrent read from clearing a mutation's flag", async () => {
    const gate = deferred<void>();
    const mutating = runBusy("install", () => gate.promise);
    await runBusy("load", async () => {});
    // The completed read must not have removed the in-flight mutation.
    expect(useBusyStore.getState().active).toContain("install");
    gate.resolve();
    await mutating;
    expect(useBusyStore.getState().active).toEqual([]);
  });

  it("removes only one instance when the same action runs twice", async () => {
    const first = deferred<void>();
    const second = deferred<void>();
    const a = runBusy("load", () => first.promise);
    const b = runBusy("load", () => second.promise);
    expect(useBusyStore.getState().active).toEqual(["load", "load"]);
    first.resolve();
    await a;
    expect(useBusyStore.getState().active).toEqual(["load"]);
    second.resolve();
    await b;
    expect(useBusyStore.getState().active).toEqual([]);
  });
});
