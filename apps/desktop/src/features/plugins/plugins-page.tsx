// One scenario's plugins: the installed (and remembered-disabled) list with
// enable/disable/update/remove per row, and the market search below to add
// more. The live operation strip streams the current install/remove/update.

import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Download, Loader2, Package, RefreshCw, Search, Trash2 } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { usePluginStore } from "@/app/store/plugins";
import { useBusyStore } from "@/app/store/busy";
import type { MarketTrust, Profile } from "@/shared/api/api";
import { cn } from "@/shared/lib/utils";
import { enterTransition, MOTION_DURATION, MOTION_EASE } from "@/shared/lib/motion";
import { Button } from "@/shared/ui/button";
import { Badge } from "@/shared/ui/badge";
import { Card, CardContent } from "@/shared/ui/card";
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
  const setPluginEnabled = usePluginStore((s) => s.setPluginEnabled);
  const runPluginOp = usePluginStore((s) => s.runPluginOp);
  const marketInstall = usePluginStore((s) => s.marketInstall);
  const dismissPluginOp = usePluginStore((s) => s.dismissPluginOp);
  const busyAction = useBusyStore((s) => s.busyAction);

  const [query, setQuery] = useState("");
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
    // The add panel opens on the curated storefront, like the old market tab.
    void searchMarket("");
  }, [loadPlugins, loadDisabledPlugins, searchMarket]);

  const rows = useMemo(
    () => scenarioPluginRows(plugins, disabledPlugins, profile.id),
    [plugins, disabledPlugins, profile.id],
  );

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
    () => filterMarket(marketEntries, category, trust),
    [marketEntries, category, trust],
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

      <section className="space-y-3">
        <h2 className="text-heading font-semibold text-text">{t("plugins.installed")}</h2>
        <motion.div
          initial={{ opacity: 0, y: 6 }}
          animate={{ opacity: 1, y: 0 }}
          transition={enterTransition}
          className="space-y-3"
        >
          {!pluginsLoaded ? (
            <Skeleton className="h-32 w-full" />
          ) : rows.length === 0 ? (
            <EmptyState icon={<Package className="h-8 w-8 text-text-faint" />}>
              {t("settings.noPluginsInScenario")}
            </EmptyState>
          ) : (
            <Card className="overflow-hidden">
              <div className="divide-y divide-border">
                <AnimatePresence initial={false}>
                  {rows.map((plugin) => {
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
                            <div className="mt-0.5 flex flex-wrap gap-x-2 text-small text-text-faint">
                              <span>{plugin.id}</span>
                              {plugin.version && (
                                <span className={plugin.outdated ? "text-warn" : undefined}>
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
                                  onClick={() => doToggle(plugin.id, true)}
                                  disabled={busy}
                                >
                                  <Download className="h-4 w-4" />
                                  {t("plugins.enableAction")}
                                </Button>
                                <Button
                                  variant="danger"
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
                                {plugin.enabled && (
                                  <Button onClick={() => doUpdate(plugin.id)} disabled={busy}>
                                    <RefreshCw className="h-4 w-4" />
                                    {t("plugins.update")}
                                  </Button>
                                )}
                                <Button
                                  variant="danger"
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
        </motion.div>
      </section>

      <section className="space-y-3">
        <div className="text-small font-bold text-text-dim">{t("settings.addPlugin")}</div>
        <Card>
          <CardContent className="flex flex-col gap-2 p-4 md:flex-row">
            <Input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder={t("settings.searchPluginPlaceholder")}
              className="w-full flex-1"
              onKeyDown={(event) => {
                if (event.key === "Enter" && query.trim()) searchMarket(query.trim());
              }}
            />
            <Button
              variant="primary"
              onClick={() => searchMarket(query.trim())}
              disabled={busy || !query.trim()}
            >
              <Search className="h-4 w-4" />
              {t("plugins.search")}
            </Button>
          </CardContent>
        </Card>

        {!pluginsLoaded ? (
          <Skeleton className="h-24 w-full" />
        ) : marketEntries.length === 0 ? (
          <EmptyState icon={<Package className="h-8 w-8 text-text-faint" />}>
            {query ? t("plugins.searchEmpty") : t("plugins.marketEmpty")}
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
                    <div key={entry.id} className="p-4">
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
                        <p className="mt-1 text-body text-text-dim">{entry.description}</p>
                      )}
                    </div>
                  );
                })}
              </div>
            </Card>
          </>
        )}
      </section>

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
