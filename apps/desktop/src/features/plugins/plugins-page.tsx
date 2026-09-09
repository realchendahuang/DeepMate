// One scenario's plugins: tabbed interface separating installed plugins
// (with search/filter, update, enable/disable) from the marketplace
// (curated/community browsing, search, category/trust filters).
// The live operation strip streams the current install/remove/update.

import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  ArrowUpRight,
  Download,
  Loader2,
  Package,
  RefreshCw,
  Search,
  Store,
  Trash2,
  X,
} from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { usePluginStore } from "@/app/store/plugins";
import { usePreferencesStore } from "@/app/store/preferences";
import { useBusyStore } from "@/app/store/busy";
import type { MarketTrust, Profile } from "@/shared/api/api";
import { cn } from "@/shared/lib/utils";
import { enterTransition, MOTION_DURATION, MOTION_EASE } from "@/shared/lib/motion";
import { Button } from "@/shared/ui/button";
import { Badge } from "@/shared/ui/badge";
import { Card } from "@/shared/ui/card";
import { EmptyState } from "@/shared/ui/empty-state";
import { ConfirmDialog } from "@/shared/ui/confirm-dialog";
import { Input } from "@/shared/ui/input";
import { Switch } from "@/shared/ui/switch";
import { Skeleton } from "@/shared/ui/skeleton";
import { TrustBadge } from "./trust-badge";
import { OpStatus } from "./op-status";
import {
  categoryFacets,
  disabledPluginKeys,
  filterMarket,
  installedPluginKeys,
  scenarioPluginRows,
  trustFacets,
} from "./catalog";

const ROW_OP_KEY = {
  install: "plugins.row.installing",
  remove: "plugins.row.removing",
  update: "plugins.row.updating",
} as const;

// The market browsing source: the curated storefront or the community
// (npm) search. Follows `Config.market.default_source` on first visit.
type MarketSourceTab = "curated" | "community";

// One pill in a market filter row (trust tier or category).
function FilterChip({
  active,
  label,
  count,
  onClick,
}: {
  active: boolean;
  label: string;
  count: number;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "rounded-full border px-3 py-1 text-small font-medium transition-colors",
        active
          ? "border-accent/40 bg-accent/10 text-accent"
          : "border-border bg-panel-2 text-text-dim hover:bg-hover",
      )}
    >
      {label}
      <span className="ml-1.5 tabular-nums opacity-70">{count}</span>
    </button>
  );
}

// A count-less pill for the source toggle (curated storefront / community
// search).
function SourceChip({
  active,
  label,
  onClick,
}: {
  active: boolean;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "rounded-full border px-3 py-1 text-small font-medium transition-colors",
        active
          ? "border-accent/40 bg-accent/10 text-accent"
          : "border-border bg-panel-2 text-text-dim hover:bg-hover",
      )}
    >
      {label}
    </button>
  );
}

export function PluginsPage({ profile }: { profile: Profile }) {
  const { t } = useTranslation();

  const plugins = usePluginStore((s) => s.plugins);
  const disabledPlugins = usePluginStore((s) => s.disabledPlugins);
  const pluginsLoaded = usePluginStore((s) => s.pluginsLoaded);
  const marketEntries = usePluginStore((s) => s.marketEntries);
  const opLog = usePluginStore((s) => s.opLog);
  const opActive = usePluginStore((s) => s.opActive);
  const loadPlugins = usePluginStore((s) => s.loadPlugins);
  const loadDisabledPlugins = usePluginStore((s) => s.loadDisabledPlugins);
  const searchMarket = usePluginStore((s) => s.searchMarket);
  const resetMarket = usePluginStore((s) => s.resetMarket);
  const setPluginEnabled = usePluginStore((s) => s.setPluginEnabled);
  const runPluginOp = usePluginStore((s) => s.runPluginOp);
  const marketInstall = usePluginStore((s) => s.marketInstall);
  const dismissPluginOp = usePluginStore((s) => s.dismissPluginOp);
  const busyAction = useBusyStore((s) => s.busyAction);

  const [activeTab, setActiveTab] = useState<"installed" | "market">("installed");
  const [filter, setFilter] = useState("");
  const [query, setQuery] = useState("");
  // The market browsing source, seeded from the persisted default source.
  const [source, setSource] = useState<MarketSourceTab>(() =>
    usePreferencesStore.getState().marketDefaultSource === "community" ? "community" : "curated",
  );
  // The row an operation was launched from, so the row shows its own running
  // state while the op streams. Cleared whenever nothing is in flight.
  const [rowOp, setRowOp] = useState<{
    id: string;
    kind: "install" | "remove" | "update";
  } | null>(null);
  // The market row an install was launched from. `marketInstall` runs the
  // compatibility preflight and is not streamed, so the spinner is local.
  const [installing, setInstalling] = useState<string | null>(null);
  const [removing, setRemoving] = useState<{ id: string; name: string } | null>(null);
  const [category, setCategory] = useState<string | null>(null);
  const [trust, setTrust] = useState<MarketTrust | null>(null);

  useEffect(() => {
    void loadPlugins();
    void loadDisabledPlugins();
  }, [loadPlugins, loadDisabledPlugins]);

  // The curated storefront loads on entry; the community tab starts empty
  // and waits for a keyword search.
  useEffect(() => {
    if (source === "community") {
      resetMarket();
    } else {
      void searchMarket("");
    }
  }, [source, searchMarket, resetMarket]);

  const rows = useMemo(
    () => scenarioPluginRows(plugins, disabledPlugins, profile.id),
    [plugins, disabledPlugins, profile.id],
  );

  const filteredRows = useMemo(() => {
    if (!filter.trim()) return rows;
    const q = filter.trim().toLowerCase();
    return rows.filter((r) => r.name.toLowerCase().includes(q) || r.id.toLowerCase().includes(q));
  }, [rows, filter]);

  const outdatedCount = useMemo(() => rows.filter((r) => r.outdated).length, [rows]);

  // Keys installed (or remembered-disabled) in this scenario, for the market
  // rows and to avoid listing market rows as "install" when they're present.
  const installedKeys = useMemo(
    () => installedPluginKeys(plugins, profile.id),
    [plugins, profile.id],
  );
  const disabledKeys = useMemo(
    () => disabledPluginKeys(disabledPlugins, profile.id),
    [disabledPlugins, profile.id],
  );

  const busy = busyAction !== null || opActive;
  const activeRowOp = busy ? rowOp : null;

  const categories = useMemo(() => categoryFacets(marketEntries), [marketEntries]);
  const trustTiers = useMemo(() => trustFacets(marketEntries), [marketEntries]);
  const visibleEntries = useMemo(
    () =>
      filterMarket(
        marketEntries,
        category,
        trust,
        // The community tab shows npm results only; the curated storefront
        // always includes its own entries at the front.
        source === "community" ? "community" : null,
      ),
    [marketEntries, category, trust, source],
  );

  const doToggle = (id: string, enabled: boolean) => {
    setRowOp({ id, kind: enabled ? "install" : "remove" });
    setPluginEnabled(profile.id, id, enabled);
  };
  const doUpdate = (id: string) => {
    setRowOp({ id, kind: "update" });
    runPluginOp(profile.id, "update", id);
  };
  const doRemove = () => {
    if (!removing) return;
    setRowOp({ id: removing.id, kind: "remove" });
    runPluginOp(profile.id, "remove", removing.id);
    setRemoving(null);
  };
  const doMarketInstall = async (id: string) => {
    setInstalling(id);
    try {
      await marketInstall(profile.id, id);
    } catch {
      // The store already surfaced the failure as a toast.
    } finally {
      setInstalling(null);
    }
  };

  return (
    <div className="space-y-4">
      <AnimatePresence initial={false}>
        {(opActive || opLog.length > 0) && (
          <motion.div
            key="op-status"
            initial={{ opacity: 0, y: -4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0 }}
            transition={{ duration: MOTION_DURATION.fast, ease: MOTION_EASE.out }}
          >
            <OpStatus log={opLog} active={opActive} onClose={dismissPluginOp} />
          </motion.div>
        )}
      </AnimatePresence>

      {/* Tabs */}
      <div className="flex items-center justify-between border-b border-border pb-3">
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => setActiveTab("installed")}
            className={cn(
              "flex items-center gap-2 rounded-lg px-3.5 py-2 text-body font-medium transition-colors",
              activeTab === "installed"
                ? "bg-accent-soft text-accent font-semibold"
                : "text-text-dim hover:bg-hover hover:text-text",
            )}
          >
            <Package className="h-4 w-4" />
            <span>{t("plugins.tabInstalled")}</span>
            <span
              className={cn(
                "ml-0.5 rounded-full px-2 py-0.5 text-micro font-semibold tabular-nums",
                activeTab === "installed"
                  ? "bg-accent/20 text-accent"
                  : "bg-panel-3 text-text-dim",
              )}
            >
              {rows.length}
            </span>
            {outdatedCount > 0 && (
              <span className="h-2 w-2 rounded-full bg-warn" title={t("plugins.outdated")} />
            )}
          </button>
          <button
            type="button"
            onClick={() => setActiveTab("market")}
            className={cn(
              "flex items-center gap-2 rounded-lg px-3.5 py-2 text-body font-medium transition-colors",
              activeTab === "market"
                ? "bg-accent-soft text-accent font-semibold"
                : "text-text-dim hover:bg-hover hover:text-text",
            )}
          >
            <Store className="h-4 w-4" />
            <span>{t("plugins.tabMarket")}</span>
          </button>
        </div>
      </div>

      {/* Installed tab */}
      {activeTab === "installed" && (
        <motion.div
          key="tab-installed"
          initial={{ opacity: 0, y: 4 }}
          animate={{ opacity: 1, y: 0 }}
          transition={enterTransition}
          className="space-y-4"
        >
          {rows.length > 0 && (
            <div className="relative">
              <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-text-faint" />
              <Input
                value={filter}
                onChange={(e) => setFilter(e.target.value)}
                placeholder={t("plugins.filterSearchPlaceholder")}
                className="pl-9 pr-8"
              />
              {filter && (
                <button
                  type="button"
                  onClick={() => setFilter("")}
                  className="absolute right-2.5 top-1/2 -translate-y-1/2 text-text-faint hover:text-text"
                >
                  <X className="h-3.5 w-3.5" />
                </button>
              )}
            </div>
          )}

          {!pluginsLoaded ? (
            <Skeleton className="h-32 w-full" />
          ) : rows.length === 0 ? (
            <Card className="p-8 text-center">
              <div className="mx-auto flex h-12 w-12 items-center justify-center rounded-xl bg-panel-3 text-text-faint">
                <Package className="h-6 w-6" />
              </div>
              <h3 className="mt-3 text-heading font-semibold text-text">
                {t("settings.noPluginsInScenario")}
              </h3>
              <p className="mt-1 text-body text-text-dim">
                {t("plugins.browseMarketHint")}
              </p>
              <div className="mt-4">
                <Button variant="primary" onClick={() => setActiveTab("market")}>
                  <Store className="h-4 w-4" />
                  {t("plugins.exploreMarket")}
                </Button>
              </div>
            </Card>
          ) : filteredRows.length === 0 ? (
            <Card className="p-8 text-center">
              <p className="text-body text-text-dim">{t("plugins.noInstalledMatches")}</p>
              <Button variant="ghost" size="sm" className="mt-3" onClick={() => setFilter("")}>
                {t("plugins.clearSearch")}
              </Button>
            </Card>
          ) : (
            <Card className="overflow-hidden">
              <div className="divide-y divide-border">
                <AnimatePresence initial={false}>
                  {filteredRows.map((plugin) => {
                    const active = activeRowOp?.id === plugin.id ? activeRowOp : null;
                    return (
                      <motion.div
                        key={`${plugin.disabled ? "d" : "p"}-${plugin.id}`}
                        layout
                        initial={{ opacity: 0 }}
                        animate={{ opacity: 1 }}
                        exit={{ opacity: 0 }}
                        transition={{ duration: MOTION_DURATION.fast, ease: MOTION_EASE.out }}
                      >
                        <div className="flex flex-col gap-3 p-4 md:flex-row md:items-center">
                          <div className="min-w-0 flex-1">
                            <div className="flex flex-wrap items-center gap-2">
                              <span className="text-heading font-semibold text-text">
                                {plugin.name}
                              </span>
                              {plugin.trust && <TrustBadge trust={plugin.trust} />}
                              <Badge variant={plugin.enabled ? "pass" : "neutral"}>
                                {plugin.enabled
                                  ? t("plugins.enabled")
                                  : t("plugins.disabled")}
                              </Badge>
                              {plugin.outdated && (
                                <Badge variant="warn">{t("plugins.outdated")}</Badge>
                              )}
                            </div>
                            <div className="mt-1 flex flex-wrap items-center gap-x-3 text-small text-text-faint">
                              <span className="font-mono text-micro">{plugin.id}</span>
                              {plugin.version && (
                                <span className={cn(plugin.outdated && "text-warn font-medium")}>
                                  {t("plugins.versionLatest", {
                                    version: plugin.version,
                                    latest: plugin.latest ?? "-",
                                  })}
                                </span>
                              )}
                            </div>
                          </div>
                          <div className="flex flex-wrap items-center gap-2">
                            {active ? (
                              <span className="flex items-center gap-2 text-small text-accent">
                                <Loader2 className="h-4 w-4 animate-spin" />
                                {t(ROW_OP_KEY[active.kind])}
                              </span>
                            ) : plugin.disabled ? (
                              <>
                                <Button
                                  variant="primary"
                                  size="sm"
                                  onClick={() => doToggle(plugin.id, true)}
                                  disabled={busy}
                                >
                                  <Download className="h-4 w-4" />
                                  {t("plugins.enableAction")}
                                </Button>
                                <Button
                                  variant="danger"
                                  size="sm"
                                  onClick={() =>
                                    setRemoving({ id: plugin.id, name: plugin.name })
                                  }
                                  disabled={busy}
                                >
                                  <Trash2 className="h-4 w-4" />
                                  {t("plugins.remove")}
                                </Button>
                              </>
                            ) : (
                              <>
                                <Switch
                                  checked={plugin.enabled}
                                  onCheckedChange={(next) => doToggle(plugin.id, next)}
                                  disabled={busy}
                                  aria-label={t(
                                    plugin.enabled
                                      ? "plugins.disableAction"
                                      : "plugins.enableAction",
                                    { name: plugin.name },
                                  )}
                                />
                                {plugin.outdated ? (
                                  <Button
                                    variant="primary"
                                    size="sm"
                                    onClick={() => doUpdate(plugin.id)}
                                    disabled={busy}
                                  >
                                    <RefreshCw className="h-4 w-4" />
                                    {t("plugins.update")}
                                  </Button>
                                ) : (
                                  <Button
                                    variant="ghost"
                                    size="sm"
                                    onClick={() => doUpdate(plugin.id)}
                                    disabled={busy}
                                  >
                                    <RefreshCw className="h-4 w-4" />
                                    {t("plugins.update")}
                                  </Button>
                                )}
                                <Button
                                  variant="danger"
                                  size="sm"
                                  onClick={() =>
                                    setRemoving({ id: plugin.id, name: plugin.name })
                                  }
                                  disabled={busy}
                                >
                                  <Trash2 className="h-4 w-4" />
                                  {t("plugins.remove")}
                                </Button>
                              </>
                            )}
                          </div>
                        </div>
                      </motion.div>
                    );
                  })}
                </AnimatePresence>
              </div>
            </Card>
          )}

          {/* Quick CTA to Marketplace */}
          {rows.length > 0 && (
            <Card
              interactive
              onClick={() => setActiveTab("market")}
              className="flex items-center justify-between p-4 cursor-pointer hover:border-accent/40"
            >
              <div className="flex items-center gap-3">
                <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-accent-soft text-accent">
                  <Store className="h-5 w-5" />
                </div>
                <div>
                  <h4 className="text-body font-semibold text-text">
                    {t("plugins.exploreMarket")}
                  </h4>
                  <p className="text-small text-text-dim">
                    {t("plugins.browseMarketHint")}
                  </p>
                </div>
              </div>
              <ArrowUpRight className="h-4 w-4 text-text-faint" />
            </Card>
          )}
        </motion.div>
      )}

      {/* Marketplace tab */}
      {activeTab === "market" && (
        <motion.div
          key="tab-market"
          initial={{ opacity: 0, y: 4 }}
          animate={{ opacity: 1, y: 0 }}
          transition={enterTransition}
          className="space-y-4"
        >
          <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div className="flex flex-wrap items-center gap-1.5">
              <SourceChip
                active={source === "curated"}
                label={t("plugins.marketSourceCurated")}
                onClick={() => setSource("curated")}
              />
              <SourceChip
                active={source === "community"}
                label={t("plugins.marketSourceCommunity")}
                onClick={() => setSource("community")}
              />
            </div>
          </div>

          <div className="relative flex items-center gap-2">
            <div className="relative flex-1">
              <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-text-faint" />
              <Input
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder={t("settings.searchPluginPlaceholder")}
                className="pl-9 pr-8"
                onKeyDown={(event) => {
                  if (event.key === "Enter" && query.trim()) searchMarket(query.trim());
                }}
              />
              {query && (
                <button
                  type="button"
                  onClick={() => {
                    setQuery("");
                    if (source === "community") resetMarket();
                    else searchMarket("");
                  }}
                  className="absolute right-2.5 top-1/2 -translate-y-1/2 text-text-faint hover:text-text"
                >
                  <X className="h-3.5 w-3.5" />
                </button>
              )}
            </div>
            <Button
              variant="primary"
              onClick={() => searchMarket(query.trim())}
              disabled={busy || !query.trim()}
            >
              <Search className="h-4 w-4" />
              {t("plugins.search")}
            </Button>
          </div>

          {!pluginsLoaded ? (
            <Skeleton className="h-24 w-full" />
          ) : marketEntries.length === 0 ? (
            <EmptyState icon={<Package className="h-8 w-8 text-text-faint" />}>
              {query
                ? t("plugins.searchEmpty")
                : source === "community"
                  ? t("plugins.marketEmptyCommunity")
                  : t("plugins.marketEmpty")}
            </EmptyState>
          ) : (
            <>
              {(trustTiers.length > 0 || categories.length > 0) && (
                <div className="flex flex-wrap items-center gap-1.5">
                  <FilterChip
                    active={trust === null}
                    label={t("plugins.all")}
                    count={marketEntries.length}
                    onClick={() => setTrust(null)}
                  />
                  {trustTiers.map(({ key, count }) => (
                    <FilterChip
                      key={key}
                      active={trust === key}
                      label={t(`plugins.trust.${key}`)}
                      count={count}
                      onClick={() => setTrust(trust === key ? null : (key as MarketTrust))}
                    />
                  ))}
                  <div className="mx-1 h-4 w-px bg-border" />
                  <FilterChip
                    active={category === null}
                    label={t("plugins.all")}
                    count={marketEntries.length}
                    onClick={() => setCategory(null)}
                  />
                  {categories.map(({ key, count }) => (
                    <FilterChip
                      key={key}
                      active={category === key}
                      label={t(`plugins.categories.${key}`, { defaultValue: key })}
                      count={count}
                      onClick={() => setCategory(category === key ? null : key)}
                    />
                  ))}
                </div>
              )}

              <Card className="overflow-hidden">
                <div className="divide-y divide-border">
                  {visibleEntries.map((entry) => {
                    const installed = installedKeys.has(entry.id);
                    const disabledHere = !installed && disabledKeys.has(entry.id);
                    return (
                      <div key={entry.id} className="p-4 hover:bg-hover/30 transition-colors">
                        <div className="flex flex-wrap items-center gap-2">
                          <span className="text-heading font-semibold text-text">{entry.id}</span>
                          <TrustBadge trust={entry.trust ?? "community"} />
                          {entry.category && (
                            <Badge variant="neutral" dot={false}>
                              {t(`plugins.categories.${entry.category}`, {
                                defaultValue: entry.category,
                              })}
                            </Badge>
                          )}
                          {entry.version && (
                            <Badge variant="skip" dot={false}>
                              v{entry.version}
                            </Badge>
                          )}
                          <span className="flex-1" />
                          {installing === entry.id ? (
                            <span className="flex items-center gap-2 text-small text-accent">
                              <Loader2 className="h-4 w-4 animate-spin" />
                              {t("plugins.row.installing")}
                            </span>
                          ) : installed ? (
                            <Badge variant="pass">{t("plugins.installedBadge")}</Badge>
                          ) : disabledHere ? (
                            <Button
                              variant="secondary"
                              size="sm"
                              disabled={busy}
                              onClick={() => doToggle(entry.id, true)}
                            >
                              <Download className="h-4 w-4" />
                              {t("plugins.enableAction")}
                            </Button>
                          ) : (
                            <Button
                              variant="primary"
                              size="sm"
                              disabled={busy}
                              title={t("settings.installToScenario")}
                              onClick={() => doMarketInstall(entry.id)}
                            >
                              <Download className="h-4 w-4" />
                              {t("plugins.install")}
                            </Button>
                          )}
                        </div>
                        {entry.description && (
                          <p className="mt-1.5 text-body text-text-dim leading-relaxed">
                            {entry.description}
                          </p>
                        )}
                      </div>
                    );
                  })}
                </div>
              </Card>
            </>
          )}
        </motion.div>
      )}

      <ConfirmDialog
        open={removing !== null}
        onOpenChange={(open) => !open && setRemoving(null)}
        title={t("plugins.removeConfirmTitle")}
        body={t("plugins.removeConfirmBody", { name: removing?.name ?? "" })}
        onConfirm={doRemove}
      />
    </div>
  );
}
