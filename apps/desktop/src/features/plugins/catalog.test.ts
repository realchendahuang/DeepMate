import { describe, expect, it } from "vitest";
import type { DisabledPlugin, MarketEntry, Plugin } from "@/shared/api/api";
import {
  categoryFacets,
  disabledPluginKeys,
  filterMarket,
  installedPluginKeys,
  scenarioPluginRows,
  trustFacets,
} from "./catalog";

const plugin = (id: string, profile: string, extra: Partial<Plugin> = {}): Plugin => ({
  id,
  name: id,
  version: "1.0.0",
  enabled: true,
  profile,
  latest: null,
  outdated: false,
  ...extra,
});

const entry = (id: string, extra: Partial<MarketEntry> = {}): MarketEntry => ({
  id,
  name: id,
  description: null,
  version: null,
  source: "curated",
  repository: null,
  publisher: null,
  updated: null,
  ...extra,
});

describe("categoryFacets", () => {
  it("counts categories in display order and skips empty tiers", () => {
    const facets = categoryFacets([
      entry("a", { category: "vision" }),
      entry("b", { category: "official" }),
      entry("c", { category: "vision" }),
      entry("d", { category: null }),
    ]);
    expect(facets).toEqual([
      { key: "official", count: 1 },
      { key: "vision", count: 2 },
    ]);
  });
});

describe("trustFacets", () => {
  it("counts trust tiers and defaults missing signals to community", () => {
    const facets = trustFacets([
      entry("a", { trust: "official" }),
      entry("b", { trust: "official" }),
      entry("c"),
    ]);
    expect(facets).toEqual([
      { key: "official", count: 2 },
      { key: "community", count: 1 },
    ]);
  });
});

describe("filterMarket", () => {
  const entries = [
    entry("a", { category: "vision", trust: "official" }),
    entry("b", { category: "mcp", trust: "community" }),
    entry("c", { category: "vision" }),
  ];

  it("passes everything through with no filters", () => {
    expect(filterMarket(entries, null, null)).toHaveLength(3);
  });

  it("filters by category and trust together", () => {
    expect(filterMarket(entries, "vision", "official")).toEqual([entries[0]]);
  });

  it("treats missing trust as community", () => {
    expect(filterMarket(entries, "vision", "community")).toEqual([entries[2]]);
  });

  it("filters by source for the community tab", () => {
    const mixed = [
      entry("a", { source: "curated" }),
      entry("b", { source: "community" }),
      entry("c", { source: "community" }),
    ];
    expect(filterMarket(mixed, null, null, "community")).toEqual([mixed[1], mixed[2]]);
    // No source filter keeps every entry.
    expect(filterMarket(mixed, null, null, null)).toHaveLength(3);
  });
});

describe("scenarioPluginRows", () => {
  it("merges installed and remembered-disabled rows for one scenario", () => {
    const disabled: DisabledPlugin[] = [
      { profile: "web", id: "dsh-memory", spec: "dsh-memory@1.0.0" },
    ];
    const rows = scenarioPluginRows(
      [plugin("dsh-base", "web"), plugin("dsh-base", "work")],
      disabled,
      "web",
    );
    expect(rows).toHaveLength(2);
    expect(rows[0]).toMatchObject({ id: "dsh-base", disabled: false, version: "1.0.0" });
    expect(rows[1]).toMatchObject({ id: "dsh-memory", disabled: true, enabled: false });
  });
});

describe("installedPluginKeys / disabledPluginKeys", () => {
  it("collects only the given scenario's keys", () => {
    expect(installedPluginKeys([plugin("a", "web"), plugin("b", "work")], "web")).toEqual(
      new Set(["a"]),
    );
    expect(
      disabledPluginKeys(
        [
          { profile: "web", id: "a", spec: "a" },
          { profile: "work", id: "b", spec: "b" },
        ],
        "work",
      ),
    ).toEqual(new Set(["b"]));
  });
});
