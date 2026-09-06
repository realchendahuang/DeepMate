// One scenario's plugin set plus its task panel, embedded in the scenario
// home page. Lists the plugins installed (or declared) in the scenario with
// their enabled state, offers enable/disable/update/remove per row, a market
// search below to add new plugins, and (for task-surface scenarios) the
// one-shot task runner.

import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  AlertTriangle,
  CheckCircle2,
  Download,
  Loader2,
  Package,
  RefreshCw,
  Search,
  Trash2,
} from "lucide-react";
import { useStore } from "../../store";
import type { MarketTrust, PluginOpEvent, Profile } from "../../api";
import { cn } from "../../lib/utils";
import { Button } from "../ui/button";
import { Badge } from "../ui/badge";
import { Card, CardContent } from "../ui/card";
import { EmptyState } from "../ui/empty-state";
import { ConfirmDialog } from "../ui/confirm-dialog";
import { Input } from "../ui/input";
import { Switch } from "../ui/switch";
import { Skeleton } from "../ui/skeleton";
import { AnimatePresence, motion } from "motion/react";
import { enterTransition, MOTION_DURATION, MOTION_EASE } from "../../lib/motion";
import { TrustBadge } from "./trust-badge";

const ROW_OP_KEY = {
  install: "plugins.row.installing",
  remove: "plugins.row.removing",
  update: "plugins.row.updating",
} as const;

// Curated storefront categories, in display order (mirrors the old global
// market page).
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
const TRUST_ORDER: MarketTrust[] = ["official", "vetted", "community"];

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

interface ScenarioPluginsProps {
  profile: Profile;
}

export function ScenarioPlugins({ profile }: ScenarioPluginsProps) {
  const { t } = useTranslation();

  const plugins = useStore((s) => s.plugins);
  const disabledPlugins = useStore((s) => s.disabledPlugins);
  const marketEntries = useStore((s) => s.marketEntries);
  const busyAction = useStore((s) => s.busyAction);
  const opLog = useStore((s) => s.opLog);
  const opActive = useStore((s) => s.opActive);
  const loadPlugins = useStore((s) => s.loadPlugins);
  const loadDisabledPlugins = useStore((s) => s.loadDisabledPlugins);
  const searchMarket = useStore((s) => s.searchMarket);
  const setPluginEnabled = useStore((s) => s.setPluginEnabled);
  const runPluginOp = useStore((s) => s.runPluginOp);
  const marketInstall = useStore((s) => s.marketInstall);
  const dismissPluginOp = useStore((s) => s.dismissPluginOp);

  const [loaded, setLoaded] = useState(false);
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
    Promise.all([loadPlugins(), loadDisabledPlugins()]).then(() => setLoaded(true));
    // The add-panel opens on the curated storefront, like the market tab.
    searchMarket("");
  }, [loadPlugins, loadDisabledPlugins, searchMarket]);

  // This scenario's plugin rows: live plugins plus disabled records, so a
  // disabled plugin stays visible with a one-click re-enable.
  const rows = useMemo(() => {
    const installed = plugins
      .filter((plugin) => plugin.profile === profile.id)
      .map((plugin) => ({ ...plugin, disabled: false }));
    const disabled = disabledPlugins
      .filter((plugin) => plugin.profile === profile.id)
      .map((plugin) => ({
        id: plugin.id,
        name: plugin.id,
        version: null,
        enabled: false,
        profile: plugin.profile,
        latest: null,
        outdated: false,
        disabled: true,
        trust: null as null,
      }));
    return [...installed, ...disabled];
  }, [plugins, disabledPlugins, profile.id]);

  // Keys installed (or remembered-disabled) in this scenario, for the market
  // rows and to avoid listing market rows as "install" when they're present.
  const installedKeys = useMemo(() => {
    const keys = new Set<string>();
    for (const plugin of plugins) {
      if (plugin.profile === profile.id) keys.add(plugin.id);
    }
    return keys;
  }, [plugins, profile.id]);
  const disabledKeys = useMemo(() => {
    const keys = new Set<string>();
    for (const plugin of disabledPlugins) {
      if (plugin.profile === profile.id) keys.add(plugin.id);
    }
    return keys;
  }, [disabledPlugins, profile.id]);

  const busy = busyAction !== null || opActive;
  const activeRowOp = busy ? rowOp : null;

  // Category and trust counts in display order; tiers with no entries hide.
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
  const trustTiers = useMemo(() => {
    const counts = new Map<MarketTrust, number>();
    for (const entry of marketEntries) {
      const tier = entry.trust ?? "community";
      counts.set(tier, (counts.get(tier) ?? 0) + 1);
    }
    return TRUST_ORDER.filter((key) => counts.has(key)).map((key) => ({
      key,
      count: counts.get(key)!,
    }));
  }, [marketEntries]);
  const visibleEntries = useMemo(() => {
    return marketEntries.filter(
      (entry) =>
        (!category || entry.category === category) &&
        (!trust || (entry.trust ?? "community") === trust),
    );
  }, [marketEntries, category, trust]);

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
        <motion.div
          initial={{ opacity: 0, y: 6 }}
          animate={{ opacity: 1, y: 0 }}
          transition={enterTransition}
          className="space-y-3"
        >
          {!loaded ? (
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
                    const active =
                      activeRowOp?.id === plugin.id ? activeRowOp : null;
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
                                  <Button
                                    onClick={() => doUpdate(plugin.id)}
                                    disabled={busy}
                                  >
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

        {!loaded ? (
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
                  onClick={() => setTrust(trust === key ? null : key)}
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

// Compact operation status for a scenario's plugin ops: same shape as the
// Plugins page strip (streams the newest harness line, keeps failures open).
function OpStatus({
  log,
  active,
  onClose,
}: {
  log: PluginOpEvent[];
  active: boolean;
  onClose: () => void;
}) {
  const { t } = useTranslation();

  const started = log.find((event) => event.phase === "started");
  const finished = log.find((event) => event.phase === "finished");
  const lines = log.filter((event) => event.phase === "line");
  const done = finished !== undefined;
  const failed = done && !finished!.ok;

  useEffect(() => {
    if (active || !finished?.ok) return;
    const timer = window.setTimeout(onClose, 1500);
    return () => window.clearTimeout(timer);
  }, [active, finished, onClose]);

  const doneKeys = {
      install: "plugins.op.installed",
      remove: "plugins.op.removed",
      update: "plugins.op.updated",
      task: "settings.taskSuccess",
    } as const;
  const status = failed
    ? t("plugins.op.failed")
    : done
      ? t(doneKeys[started?.op ?? "update"], { target: started?.target ?? "" })
      : started
        ? t(`plugins.op.${started.op}`, { target: started.target })
        : t("plugins.op.inProgress");

  return (
    <Card className="rounded-lg border border-accent/40">
      <CardContent className="p-3">
        <div className="flex items-center gap-2">
          {failed ? (
            <AlertTriangle className="h-4 w-4 shrink-0 text-fail" />
          ) : done ? (
            <CheckCircle2 className="h-4 w-4 shrink-0 text-pass" />
          ) : (
            <Loader2 className="h-4 w-4 shrink-0 animate-spin text-accent" />
          )}
          <span className="min-w-0 flex-1 truncate text-small font-medium text-text">{status}</span>
          {failed && (
            <Button variant="ghost" size="sm" onClick={onClose}>
              {t("plugins.op.close")}
            </Button>
          )}
        </div>
        {failed && (
          <div className="mt-2 max-h-40 overflow-y-auto rounded-md border border-border bg-inset p-2 font-mono text-caption text-text-dim">
            {lines.map((event, index) => (
              <div key={index} className="whitespace-pre-wrap break-words">
                {event.text}
              </div>
            ))}
            {finished?.detail && (
              <div className="mt-1 whitespace-pre-wrap break-words text-warn">
                {finished.detail}
              </div>
            )}
          </div>
        )}
      </CardContent>
    </Card>
  );
}