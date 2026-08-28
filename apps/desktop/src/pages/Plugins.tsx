import { useEffect, useState } from "react";
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
  }, [loadPlugins, loadMarketSources]);

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

          {marketEntries.length === 0 ? (
            <EmptyState icon={<ShoppingBag className="h-8 w-8 text-text-faint" />}>
              {t("plugins.searchEmpty")}
            </EmptyState>
          ) : (
            <Card className="overflow-hidden">
              <div className="divide-y divide-border">
                {marketEntries.map((entry) => (
                  <div key={entry.id} className="p-4">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="text-heading font-semibold text-text">{entry.id}</span>
                      <Badge variant={entry.source === "curated" ? "accent" : "neutral"}>{entry.source}</Badge>
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
