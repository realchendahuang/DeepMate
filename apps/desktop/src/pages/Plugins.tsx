import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Download, RefreshCw, Trash2, Search, Package, ShoppingBag } from "lucide-react";
import { useStore } from "../store";
import { Button } from "../components/ui/button";
import { Card, CardContent } from "../components/ui/card";
import { Badge } from "../components/ui/badge";
import { EmptyState } from "../components/ui/data-list";
import { PageBody, PageHeader } from "../components/ui/page";
import { Segmented } from "../components/ui/segmented";
import { Input } from "../components/ui/input";

// Curated storefront categories, in display order. Keys map to
// `plugins.categories.*` in the locale files; unknown categories fall back
// to their raw key.
const CATEGORY_ORDER = [
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
];

export function PluginsPage() {
  const { t } = useTranslation();
  const [tab, setTab] = useState(0);

  return (
    <PageBody className="space-y-5">
      <PageHeader
        title={t("plugins.title")}
        actions={
          <Segmented
            options={[t("plugins.installed"), t("plugins.market")]}
            value={tab}
            onChange={setTab}
          />
        }
      />
      <PluginsContent tab={tab} onTabChange={setTab} />
    </PageBody>
  );
}

interface PluginsContentProps {
  tab: number;
  onTabChange: (index: number) => void;
}

export function PluginsContent({ tab }: PluginsContentProps) {
  const { t } = useTranslation();
  const [installProfile, setInstallProfile] = useState("web");
  const [installSpec, setInstallSpec] = useState("");
  const [query, setQuery] = useState("");
  // Active category filter on the market tab; null shows every entry.
  const [category, setCategory] = useState<string | null>(null);

  const plugins = useStore((s) => s.plugins);
  const marketSources = useStore((s) => s.marketSources);
  const marketEntries = useStore((s) => s.marketEntries);
  const busy = useStore((s) => s.busy);
  const loadPlugins = useStore((s) => s.loadPlugins);
  const loadMarketSources = useStore((s) => s.loadMarketSources);
  const searchMarket = useStore((s) => s.searchMarket);
  const installPlugin = useStore((s) => s.installPlugin);
  const marketInstall = useStore((s) => s.marketInstall);
  const removePlugin = useStore((s) => s.removePlugin);
  const updatePlugin = useStore((s) => s.updatePlugin);

  useEffect(() => {
    loadPlugins();
    loadMarketSources();
    // The market tab opens on the curated storefront: an empty query shows
    // the curated list without hitting the npm search.
    searchMarket("");
  }, [loadPlugins, loadMarketSources, searchMarket]);

  // Category chips are derived from the curated entries, in display order.
  // npm search results carry no category and only appear under "All".
  const categories = useMemo(() => {
    const counts = new Map<string, number>();
    for (const entry of marketEntries) {
      if (!entry.category) continue;
      counts.set(entry.category, (counts.get(entry.category) ?? 0) + 1);
    }
    return CATEGORY_ORDER.filter((key) => counts.has(key)).map((key) => ({
      key,
      count: counts.get(key)!,
    }));
  }, [marketEntries]);

  const visibleEntries = useMemo(() => {
    if (!category) return marketEntries;
    return marketEntries.filter((entry) => entry.category === category);
  }, [marketEntries, category]);

  const doInstall = () => {
    if (installSpec) installPlugin(installProfile, installSpec);
  };
  const doSearch = () => {
    if (query) searchMarket(query);
  };

  return (
    <>
      {tab === 0 && (
        <div className="space-y-4">
          <Card>
            <CardContent className="flex flex-col gap-2 p-4 md:flex-row">
              <Input
                value={installProfile}
                onChange={(event) => setInstallProfile(event.target.value)}
                placeholder={t("plugins.profile")}
                className="w-full md:w-[140px]"
                onKeyDown={(event) => event.key === "Enter" && doInstall()}
              />
              <Input
                value={installSpec}
                onChange={(event) => setInstallSpec(event.target.value)}
                placeholder={t("plugins.packageName")}
                className="w-full flex-1 md:w-auto"
                onKeyDown={(event) => event.key === "Enter" && doInstall()}
              />
              <Button variant="primary" onClick={doInstall} disabled={busy || !installSpec}>
                <Download className="h-4 w-4" />
                {t("plugins.install")}
              </Button>
            </CardContent>
          </Card>

          {plugins.length === 0 ? (
            <EmptyState icon={<Package className="h-8 w-8 text-text-faint" />}>
              {t("plugins.noPlugins")}
            </EmptyState>
          ) : (
            <Card className="overflow-hidden">
              <div className="divide-y divide-border">
                {plugins.map((plugin) => (
                  <div key={plugin.id} className="flex flex-col gap-3 p-4 md:flex-row md:items-center">
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="text-heading font-semibold text-text">{plugin.name}</span>
                        <Badge variant={plugin.enabled ? "pass" : "neutral"}>
                          {plugin.enabled ? t("plugins.enabled") : t("plugins.disabled")}
                        </Badge>
                        {plugin.outdated && <Badge variant="warn">{t("plugins.outdated")}</Badge>}
                      </div>
                      <div className="mt-0.5 flex flex-wrap gap-x-2 text-small text-text-faint">
                        <span>{plugin.profile}/{plugin.id}</span>
                        {plugin.version && (
                          <span className={plugin.outdated ? "text-warn" : undefined}>
                            {t("plugins.versionLatest", { version: plugin.version, latest: plugin.latest ?? "-" })}
                          </span>
                        )}
                      </div>
                    </div>
                    <div className="flex flex-wrap gap-2">
                      <Button onClick={() => updatePlugin(plugin.profile, plugin.id)} disabled={busy}>
                        <RefreshCw className="h-4 w-4" />
                        {t("plugins.update")}
                      </Button>
                      <Button
                        variant="danger"
                        onClick={() => removePlugin(plugin.profile, plugin.id)}
                        disabled={busy}
                      >
                        <Trash2 className="h-4 w-4" />
                        {t("plugins.remove")}
                      </Button>
                    </div>
                  </div>
                ))}
              </div>
            </Card>
          )}
        </div>
      )}

      {tab === 1 && (
        <div className="space-y-4">
          <Card>
            <CardContent className="flex flex-col gap-2 p-4 md:flex-row">
              <Input
                value={installProfile}
                onChange={(event) => setInstallProfile(event.target.value)}
                placeholder={t("plugins.profile")}
                className="w-full md:w-[140px]"
                onKeyDown={(event) => event.key === "Enter" && doSearch()}
              />
              <Input
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder={t("plugins.searchTerms")}
                className="w-full flex-1 md:w-auto"
                onKeyDown={(event) => event.key === "Enter" && doSearch()}
              />
              <Button variant="primary" onClick={doSearch} disabled={busy || !query}>
                <Search className="h-4 w-4" />
                {t("plugins.search")}
              </Button>
            </CardContent>
          </Card>

          {categories.length > 0 && (
            <div className="flex flex-wrap items-center gap-1.5">
              <button
                type="button"
                onClick={() => setCategory(null)}
                className={`rounded-full border px-3 py-1 text-small font-medium transition-colors ${
                  category === null
                    ? "border-accent/40 bg-accent/10 text-accent"
                    : "border-border bg-panel-2 text-text-dim hover:bg-hover"
                }`}
              >
                {t("plugins.all")}
                <span className="ml-1.5 tabular-nums opacity-70">{marketEntries.length}</span>
              </button>
              {categories.map(({ key, count }) => (
                <button
                  key={key}
                  type="button"
                  onClick={() => setCategory(category === key ? null : key)}
                  className={`rounded-full border px-3 py-1 text-small font-medium transition-colors ${
                    category === key
                      ? "border-accent/40 bg-accent/10 text-accent"
                      : "border-border bg-panel-2 text-text-dim hover:bg-hover"
                  }`}
                >
                  {t(`plugins.categories.${key}`, { defaultValue: key })}
                  <span className="ml-1.5 tabular-nums opacity-70">{count}</span>
                </button>
              ))}
            </div>
          )}

          {marketSources.length > 0 && (
            <Card>
              <CardContent className="p-4">
                <div className="mb-2 text-small font-bold text-text-dim">{t("plugins.sources")}</div>
                <div className="space-y-1.5">
                  {marketSources.map((source) => (
                    <div key={source.id} className="flex flex-wrap items-center gap-2">
                      <Badge variant={source.source === "curated" ? "accent" : "neutral"}>{source.name}</Badge>
                      {source.description && <span className="text-small text-text-dim">{source.description}</span>}
                    </div>
                  ))}
                </div>
              </CardContent>
            </Card>
          )}

          {visibleEntries.length === 0 ? (
            <EmptyState icon={<ShoppingBag className="h-8 w-8 text-text-faint" />}>
              {query ? t("plugins.searchEmpty") : t("plugins.marketEmpty")}
            </EmptyState>
          ) : (
            <Card className="overflow-hidden">
              <div className="divide-y divide-border">
                {visibleEntries.map((entry) => (
                  <div key={entry.id} className="p-4">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="text-heading font-semibold text-text">{entry.id}</span>
                      <TrustBadge source={entry.source} category={entry.category} />
                      {entry.category && (
                        <Badge variant="neutral" dot={false}>
                          {t(`plugins.categories.${entry.category}`, { defaultValue: entry.category })}
                        </Badge>
                      )}
                      {entry.version && <Badge variant="skip" dot={false}>v{entry.version}</Badge>}
                      {entry.publisher && <span className="text-small text-text-faint">{t("plugins.by", { publisher: entry.publisher })}</span>}
                      <span className="flex-1" />
                      <Button
                        variant="primary"
                        size="sm"
                        disabled={busy || !installProfile.trim()}
                        title={
                          installProfile.trim()
                            ? t("plugins.installInto", { profile: installProfile.trim() })
                            : t("plugins.profileRequired")
                        }
                        onClick={() => marketInstall(installProfile.trim(), entry.id)}
                      >
                        <Download className="h-4 w-4" />
                        {t("plugins.install")}
                      </Button>
                    </div>
                    {entry.description && <p className="mt-1 text-body text-text-dim">{entry.description}</p>}
                    <div className="mt-1.5 flex flex-wrap items-center gap-x-4 gap-y-1.5">
                      {entry.repository && <p className="max-w-full truncate text-small text-accent">{entry.repository}</p>}
                      {entry.updated && (
                        <span className="text-small text-text-faint">
                          {t("plugins.updated", { date: entry.updated.slice(0, 10) })}
                        </span>
                      )}
                      <TrustMeter label={t("plugins.popularity")} value={entry.popularity} />
                      <TrustMeter label={t("plugins.quality")} value={entry.quality} />
                    </div>
                  </div>
                ))}
              </div>
            </Card>
          )}
        </div>
      )}
    </>
  );
}

// Source trust badge: curated entries are split into official (DeepSeek
// Harness vendor, category "official") and vetted (reviewed by the DeepMate
// maintainers); npm search results are community.
function TrustBadge({ source, category }: { source: "curated" | "community"; category: string | null }) {
  const { t } = useTranslation();
  if (source === "community") {
    return <Badge variant="neutral">{t("plugins.trust.community")}</Badge>;
  }
  if (category === "official") {
    return <Badge variant="accent">{t("plugins.trust.official")}</Badge>;
  }
  return <Badge variant="accent">{t("plugins.trust.vetted")}</Badge>;
}

// A compact 0-100 meter for a registry trust score; rendered only when the
// registry published the signal.
function TrustMeter({ label, value }: { label: string; value: number | null }) {
  if (value == null) return null;
  const percent = Math.round(value * 100);
  return (
    <span className="flex items-center gap-1.5 text-small text-text-faint" title={`${label}: ${percent}%`}>
      <span>{label}</span>
      <span className="h-1.5 w-14 overflow-hidden rounded-full bg-inset">
        <span className="block h-full rounded-full bg-accent" style={{ width: `${percent}%` }} />
      </span>
      <span className="tabular-nums">{percent}</span>
    </span>
  );
}
