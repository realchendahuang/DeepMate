import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Download, RefreshCw, Trash2, Search, Package, ShoppingBag, Loader2 } from "lucide-react";
import { useStore } from "../store";
import { Button } from "../components/ui/button";
import { Card, CardContent } from "../components/ui/card";
import { Badge } from "../components/ui/badge";
import { Skeleton } from "../components/ui/skeleton";
import { EmptyState } from "../components/ui/empty-state";
import { ConfirmDialog } from "../components/ui/confirm-dialog";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../components/ui/dialog";
import { PageBody, PageHeader } from "../components/ui/page";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "../components/ui/tabs";
import { Input } from "../components/ui/input";
import { cn } from "../lib/utils";
import type { PluginOpEvent } from "../api";

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
  const [tab, setTab] = useState("installed");

  return (
    <PageBody className="space-y-5">
      {/* The Tabs root must wrap the content panels: Radix throws when a
          TabsContent renders outside its Root, which blanks the whole app
          (there is no error boundary on this route). */}
      <Tabs value={tab} onValueChange={setTab} className="gap-5">
        <PageHeader
          title={t("plugins.title")}
          actions={
            <TabsList>
              <TabsTrigger value="installed">{t("plugins.installed")}</TabsTrigger>
              <TabsTrigger value="market">{t("plugins.market")}</TabsTrigger>
            </TabsList>
          }
        />
        <PluginsContent onTabChange={setTab} />
      </Tabs>
    </PageBody>
  );
}

interface PluginsContentProps {
  onTabChange: (tab: string) => void;
}

export function PluginsContent({ onTabChange }: PluginsContentProps) {
  const { t } = useTranslation();
  const [installProfile, setInstallProfile] = useState("web");
  const [installSpec, setInstallSpec] = useState("");
  const [query, setQuery] = useState("");
  // Active category filter on the market tab; null shows every entry.
  const [category, setCategory] = useState<string | null>(null);
  // Filter for the installed list.
  const [installedQuery, setInstalledQuery] = useState("");
  // Pending removal confirmation.
  const [removing, setRemoving] = useState<{ profile: string; id: string; name: string } | null>(
    null,
  );

  const plugins = useStore((s) => s.plugins);
  const marketSources = useStore((s) => s.marketSources);
  const marketEntries = useStore((s) => s.marketEntries);
  const busyAction = useStore((s) => s.busyAction);
  const loadPlugins = useStore((s) => s.loadPlugins);
  const loadMarketSources = useStore((s) => s.loadMarketSources);
  const searchMarket = useStore((s) => s.searchMarket);
  const runPluginOp = useStore((s) => s.runPluginOp);
  const dismissPluginOp = useStore((s) => s.dismissPluginOp);
  const opLog = useStore((s) => s.opLog);
  const opActive = useStore((s) => s.opActive);

  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    Promise.all([loadPlugins(), loadMarketSources()]).then(() => setLoaded(true));
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

  // Installed plugins, filtered by the local search box.
  const visiblePlugins = useMemo(() => {
    const q = installedQuery.trim().toLowerCase();
    if (!q) return plugins;
    return plugins.filter(
      (plugin) =>
        plugin.name.toLowerCase().includes(q) ||
        plugin.id.toLowerCase().includes(q) ||
        plugin.profile.toLowerCase().includes(q),
    );
  }, [plugins, installedQuery]);

  // Installed plugin ids per profile, for the market "installed" badge.
  const installedKeys = useMemo(() => {
    const keys = new Set<string>();
    for (const plugin of plugins) keys.add(`${plugin.profile}/${plugin.id}`);
    return keys;
  }, [plugins]);

  const doInstall = () => {
    if (installSpec) runPluginOp(installProfile, "install", installSpec);
  };
  const doSearch = () => {
    if (query) searchMarket(query);
  };
  const doUpdate = (profile: string, id: string) => {
    runPluginOp(profile, "update", id);
  };
  const doRemove = (profile: string, id: string) => {
    runPluginOp(profile, "remove", id);
  };

  const busy = busyAction !== null;

  return (
    <>
      <TabsContent value="installed" className="space-y-4">
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

        {plugins.length > 0 && (
          <div className="flex items-center gap-2">
            <Search className="h-4 w-4 shrink-0 text-text-faint" />
            <Input
              value={installedQuery}
              onChange={(event) => setInstalledQuery(event.target.value)}
              placeholder={t("plugins.searchInstalled")}
              className="max-w-xs"
            />
          </div>
        )}

        {!loaded ? (
          <Skeleton className="h-32 w-full" />
        ) : visiblePlugins.length === 0 ? (
          <EmptyState
            icon={<Package className="h-8 w-8 text-text-faint" />}
            action={
              <Button variant="secondary" size="sm" onClick={() => onTabChange("market")}>
                <ShoppingBag className="h-4 w-4" />
                {t("plugins.goToMarket")}
              </Button>
            }
          >
            {plugins.length === 0 ? t("plugins.noPlugins") : t("plugins.searchEmpty")}
          </EmptyState>
        ) : (
          <Card className="overflow-hidden">
            <div className="divide-y divide-border">
              {visiblePlugins.map((plugin) => (
                <div
                  key={plugin.id}
                  className="flex flex-col gap-3 p-4 md:flex-row md:items-center"
                >
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="text-heading font-semibold text-text">{plugin.name}</span>
                      <Badge variant={plugin.enabled ? "pass" : "neutral"}>
                        {plugin.enabled ? t("plugins.enabled") : t("plugins.disabled")}
                      </Badge>
                      {plugin.outdated && <Badge variant="warn">{t("plugins.outdated")}</Badge>}
                    </div>
                    <div className="mt-0.5 flex flex-wrap gap-x-2 text-small text-text-faint">
                      <span>
                        {plugin.profile}/{plugin.id}
                      </span>
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
                  <div className="flex flex-wrap gap-2">
                    <Button onClick={() => doUpdate(plugin.profile, plugin.id)} disabled={busy}>
                      <RefreshCw className="h-4 w-4" />
                      {t("plugins.update")}
                    </Button>
                    <Button
                      variant="danger"
                      onClick={() =>
                        setRemoving({ profile: plugin.profile, id: plugin.id, name: plugin.name })
                      }
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
      </TabsContent>

      <TabsContent value="market" className="space-y-4">
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
              className={cn(
                "rounded-full border px-3 py-1 text-small font-medium transition-colors",
                category === null
                  ? "border-accent/40 bg-accent/10 text-accent"
                  : "border-border bg-panel-2 text-text-dim hover:bg-hover",
              )}
            >
              {t("plugins.all")}
              <span className="ml-1.5 tabular-nums opacity-70">{marketEntries.length}</span>
            </button>
            {categories.map(({ key, count }) => (
              <button
                key={key}
                type="button"
                onClick={() => setCategory(category === key ? null : key)}
                className={cn(
                  "rounded-full border px-3 py-1 text-small font-medium transition-colors",
                  category === key
                    ? "border-accent/40 bg-accent/10 text-accent"
                    : "border-border bg-panel-2 text-text-dim hover:bg-hover",
                )}
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
                    <Badge variant={source.source === "curated" ? "accent" : "neutral"}>
                      {source.name}
                    </Badge>
                    {source.description && (
                      <span className="text-small text-text-dim">{source.description}</span>
                    )}
                  </div>
                ))}
              </div>
            </CardContent>
          </Card>
        )}

        {!loaded ? (
          <Skeleton className="h-32 w-full" />
        ) : visibleEntries.length === 0 ? (
          <EmptyState icon={<ShoppingBag className="h-8 w-8 text-text-faint" />}>
            {query ? t("plugins.searchEmpty") : t("plugins.marketEmpty")}
          </EmptyState>
        ) : (
          <>
            {query && (
              <p className="text-small text-text-dim">
                {t("plugins.results", { count: visibleEntries.length })}
              </p>
            )}
            <Card className="overflow-hidden">
              <div className="divide-y divide-border">
                {visibleEntries.map((entry) => {
                  const installed = installedKeys.has(`${installProfile.trim()}/${entry.id}`);
                  return (
                    <div key={entry.id} className="p-4">
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="text-heading font-semibold text-text">{entry.id}</span>
                        <TrustBadge source={entry.source} category={entry.category} />
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
                        {entry.publisher && (
                          <span className="text-small text-text-faint">
                            {t("plugins.by", { publisher: entry.publisher })}
                          </span>
                        )}
                        <span className="flex-1" />
                        {installed ? (
                          <Badge variant="pass">{t("plugins.installedBadge")}</Badge>
                        ) : (
                          <Button
                            variant="primary"
                            size="sm"
                            disabled={busy || !installProfile.trim()}
                            title={
                              installProfile.trim()
                                ? t("plugins.installInto", { profile: installProfile.trim() })
                                : t("plugins.profileRequired")
                            }
                            onClick={() => runPluginOp(installProfile.trim(), "install", entry.id)}
                          >
                            <Download className="h-4 w-4" />
                            {t("plugins.install")}
                          </Button>
                        )}
                      </div>
                      {entry.description && (
                        <p className="mt-1 text-body text-text-dim">{entry.description}</p>
                      )}
                      <div className="mt-1.5 flex flex-wrap items-center gap-x-4 gap-y-1.5">
                        {entry.repository && (
                          <p className="max-w-full truncate text-small text-accent">
                            {entry.repository}
                          </p>
                        )}
                        {entry.updated && (
                          <span className="text-small text-text-faint">
                            {t("plugins.updated", { date: entry.updated.toLocaleDateString() })}
                          </span>
                        )}
                        <TrustMeter label={t("plugins.popularity")} value={entry.popularity} />
                        <TrustMeter label={t("plugins.quality")} value={entry.quality} />
                      </div>
                    </div>
                  );
                })}
              </div>
            </Card>
          </>
        )}
      </TabsContent>

      <ConfirmDialog
        open={removing !== null}
        onOpenChange={(open) => !open && setRemoving(null)}
        title={t("plugins.removeConfirmTitle")}
        body={t("plugins.removeConfirmBody", { name: removing?.name ?? "" })}
        onConfirm={() => {
          if (removing) doRemove(removing.profile, removing.id);
        }}
      />

      <OpProgressDialog
        open={opActive || opLog.length > 0}
        onClose={dismissPluginOp}
        log={opLog}
        active={opActive}
      />
    </>
  );
}

// Live progress log for a plugin operation (install / remove / update). The
// dialog stays open while the operation runs and shows the harness output
// lines as they stream in; once finished it shows the outcome and a close
// button.
function OpProgressDialog({
  open,
  onClose,
  log,
  active,
}: {
  open: boolean;
  onClose: () => void;
  log: PluginOpEvent[];
  active: boolean;
}) {
  const { t } = useTranslation();
  const scrollRef = useRef<HTMLDivElement>(null);

  // Keep the newest lines in view as they stream in.
  useEffect(() => {
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [log]);

  const started = log.find((event) => event.phase === "started");
  const finished = log.find((event) => event.phase === "finished");
  const lines = log.filter((event) => event.phase === "line");

  const title = started
    ? t(`plugins.op.${started.op}`, { target: started.target })
    : t("plugins.op.running");
  const done = finished !== undefined;

  return (
    <Dialog open={open} onOpenChange={(next) => !next && !active && onClose()}>
      <DialogContent className="max-w-lg" showCloseButton={!active}>
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            {!done && <Loader2 className="h-4 w-4 animate-spin text-accent" />}
            {title}
          </DialogTitle>
          <DialogDescription>
            {done
              ? finished?.ok
                ? t("plugins.op.completed")
                : t("plugins.op.failed")
              : t("plugins.op.inProgress")}
          </DialogDescription>
        </DialogHeader>
        <div
          ref={scrollRef}
          className="max-h-56 overflow-y-auto rounded-md border border-border bg-inset p-3 font-mono text-small text-text-dim"
        >
          {lines.length === 0 ? (
            <span className="text-text-faint">{t("plugins.op.noOutput")}</span>
          ) : (
            lines.map((event, index) => (
              <div key={index} className="whitespace-pre-wrap break-words">
                {event.text}
              </div>
            ))
          )}
          {finished && !finished.ok && finished.detail && (
            <div className="mt-2 whitespace-pre-wrap break-words text-warn">{finished.detail}</div>
          )}
        </div>
        <DialogFooter>
          {done && (
            <Button variant="primary" onClick={onClose}>
              {t("plugins.op.close")}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// Source trust badge: curated entries are split into official (DeepSeek
// Harness vendor, category "official") and vetted (reviewed by the DeepMate
// maintainers); npm search results are community.
function TrustBadge({
  source,
  category,
}: {
  source: "curated" | "community";
  category?: string | null;
}) {
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
function TrustMeter({ label, value }: { label: string; value?: number | null }) {
  if (value == null) return null;
  const percent = Math.round(value * 100);
  return (
    <span
      className="flex items-center gap-1.5 text-small text-text-faint"
      title={`${label}: ${percent}%`}
    >
      <span>{label}</span>
      <span className="h-1.5 w-14 overflow-hidden rounded-full bg-inset">
        <span className="block h-full rounded-full bg-accent" style={{ width: `${percent}%` }} />
      </span>
      <span className="tabular-nums">{percent}</span>
    </span>
  );
}
