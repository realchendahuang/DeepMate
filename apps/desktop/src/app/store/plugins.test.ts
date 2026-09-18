// The market search must not let a slow answer overwrite a newer one.
//
// Two things make this worth a regression test rather than reasoning: the
// store is a single mutable object shared by the page, and the sequence
// number that guards it is easy to drop in a refactor (it was added after a
// reported bug where switching source mid-search showed the wrong list).

import { beforeEach, describe, expect, it, vi } from "vitest";

// The store imports the Tauri-backed api module; stub it so the tests drive
// the promise timing themselves.
const searchMarket = vi.fn();
vi.mock("@/shared/api/api", () => ({
  api: {
    marketSearch: (query: string) => searchMarket(query),
    listPlugins: vi.fn().mockResolvedValue([]),
    listDisabledPlugins: vi.fn().mockResolvedValue([]),
  },
}));

// runBusy toasts on failure and the store rethrows; the toast path touches
// i18n, which is not needed here.
vi.mock("sonner", () => ({ toast: { error: vi.fn(), success: vi.fn() } }));
vi.mock("@/i18n", () => ({ default: { t: (key: string) => key } }));

import { usePluginStore } from "./plugins";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const entry = (id: string) => ({
  id,
  name: id,
  description: "",
  version: "1.0.0",
  source: "community" as const,
  trust: "community" as const,
  repository: null,
  publisher: null,
  updated: null,
  category: null,
  popularity: null,
  quality: null,
});

describe("market search sequencing", () => {
  beforeEach(() => {
    searchMarket.mockReset();
    usePluginStore.setState({ marketEntries: [], marketSeq: 0, marketError: null });
  });

  it("keeps the newest result when an older search answers later", async () => {
    const slow = deferred<ReturnType<typeof entry>[]>();
    const fast = deferred<ReturnType<typeof entry>[]>();
    searchMarket.mockReturnValueOnce(slow.promise).mockReturnValueOnce(fast.promise);

    const first = usePluginStore.getState().searchMarket("old");
    const second = usePluginStore.getState().searchMarket("new");

    // The second search answers first, then the first one straggles in.
    fast.resolve([entry("new-result")]);
    await second;
    slow.resolve([entry("stale-result")]);
    await first;

    expect(usePluginStore.getState().marketEntries.map((e) => e.id)).toEqual(["new-result"]);
  });

  it("an in-flight search cannot repopulate a cleared list", async () => {
    const pending = deferred<ReturnType<typeof entry>[]>();
    searchMarket.mockReturnValueOnce(pending.promise);

    const inFlight = usePluginStore.getState().searchMarket("dsh");
    // The user switches market source, which clears the list.
    usePluginStore.getState().resetMarket();
    pending.resolve([entry("late-result")]);
    await inFlight;

    expect(usePluginStore.getState().marketEntries).toEqual([]);
  });

  it("records a failure so the page can offer a retry", async () => {
    searchMarket.mockRejectedValueOnce(new Error("offline"));
    await expect(usePluginStore.getState().searchMarket("dsh")).rejects.toThrow("offline");
    expect(usePluginStore.getState().marketError).toContain("offline");
    expect(usePluginStore.getState().marketSearching).toBe(false);
  });
});
