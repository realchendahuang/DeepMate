// Pure helpers for the plugin catalogue: the installed/disabled row merge,
// the market facets (trust tiers, categories) and the entry filter. Kept
// side-effect free so the table logic stays testable outside React.

import type { DisabledPlugin, MarketEntry, MarketTrust, Plugin } from "@/shared/api/api";

// Curated storefront categories, in display order.
export const CATEGORY_ORDER = [
  "official",
  "memory",
  "vision",
  "mcp",
  "chat",
  "web",
  "web-ui",
  "terminal",
  "tui",
  "remote",
  "office",
  "dev",
  "stats",
  "security",
  "automation",
] as const;

export const TRUST_ORDER: readonly MarketTrust[] = ["official", "vetted", "community"];

export interface Facet {
  key: string;
  count: number;
}

// Category counts in display order; categories with no entries hide.
export function categoryFacets(entries: MarketEntry[]): Facet[] {
  const counts = new Map<string, number>();
  for (const entry of entries) {
    if (!entry.category) continue;
    counts.set(entry.category, (counts.get(entry.category) ?? 0) + 1);
  }
  return CATEGORY_ORDER.filter((key) => counts.has(key)).map((key) => ({
    key,
    count: counts.get(key)!,
  }));
}

// Trust-tier counts in display order; tiers with no entries hide. Entries
// without a trust signal fall back to "community".
export function trustFacets(entries: MarketEntry[]): Facet[] {
  const counts = new Map<MarketTrust, number>();
  for (const entry of entries) {
    const tier = entry.trust ?? "community";
    counts.set(tier, (counts.get(tier) ?? 0) + 1);
  }
  return TRUST_ORDER.filter((key) => counts.has(key)).map((key) => ({
    key,
    count: counts.get(key)!,
  }));
}

export function filterMarket(
  entries: MarketEntry[],
  category: string | null,
  trust: MarketTrust | null,
  source?: MarketEntry["source"] | null,
): MarketEntry[] {
  return entries.filter(
    (entry) =>
      (!category || entry.category === category) &&
      (!trust || (entry.trust ?? "community") === trust) &&
      (!source || entry.source === source),
  );
}

// One scenario's plugin rows: installed plugins plus remembered disabled
// records, so a disabled plugin stays visible with a one-click re-enable.
export interface PluginRow {
  id: string;
  name: string;
  version: string | null;
  enabled: boolean;
  profile: string;
  latest: string | null;
  outdated: boolean;
  disabled: boolean;
  trust?: Plugin["trust"];
}

export function scenarioPluginRows(
  plugins: Plugin[],
  disabledPlugins: DisabledPlugin[],
  profile: string,
): PluginRow[] {
  const installed = plugins
    .filter((plugin) => plugin.profile === profile)
    .map((plugin) => ({ ...plugin, disabled: false }));
  const disabled = disabledPlugins
    .filter((plugin) => plugin.profile === profile)
    .map((plugin) => ({
      id: plugin.id,
      name: plugin.id,
      version: null,
      enabled: false,
      profile: plugin.profile,
      latest: null,
      outdated: false,
      disabled: true,
      trust: null as Plugin["trust"],
    }));
  return [...installed, ...disabled];
}

export function installedPluginKeys(plugins: Plugin[], profile: string): Set<string> {
  const keys = new Set<string>();
  for (const plugin of plugins) {
    if (plugin.profile === profile) keys.add(plugin.id);
  }
  return keys;
}

export function disabledPluginKeys(
  disabledPlugins: DisabledPlugin[],
  profile: string,
): Set<string> {
  const keys = new Set<string>();
  for (const plugin of disabledPlugins) {
    if (plugin.profile === profile) keys.add(plugin.id);
  }
  return keys;
}
