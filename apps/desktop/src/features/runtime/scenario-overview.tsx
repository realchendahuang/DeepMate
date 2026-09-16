// A scenario's home: identity hero card with runtime controls,
// a full dashboard metrics grid (runtime, models, plugins, engine health),
// and (for task-surface scenarios) the one-shot task runner.

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Activity,
  ArrowUpRight,
  Check,
  Cloud,
  Copy,
  ExternalLink,
  FileText,
  Puzzle,
  RotateCw,
  ShieldCheck,
  SquarePen,
  Trash2,
} from "lucide-react";
import { useRouterStore } from "@/app/router";
import { useRuntimeStore } from "@/app/store/runtime";
import { useProviderStore } from "@/app/store/providers";
import { usePluginStore } from "@/app/store/plugins";
import type { Profile } from "@/shared/api/api";
import { cn } from "@/shared/lib/utils";
import {
  inferSurface,
  isDefaultScenario,
  scenarioInitial,
  scenarioLayers,
} from "@/shared/lib/scenario";
import { Card, CardContent } from "@/shared/ui/card";
import { Button } from "@/shared/ui/button";
import { Badge } from "@/shared/ui/badge";
import { PageBody } from "@/shared/ui/page";
import { SurfaceBadge } from "@/features/scenarios/surface-badge";
import { ScenarioManageDialogs, useScenarioManage } from "@/features/scenarios/scenario-manage";
import { WebRuntimeControls } from "./runtime-controls";
import { TaskRunner } from "./task-runner";
import { EngineMissingBanner } from "./engine-banner";

export function ScenarioOverview({ profile }: { profile: Profile }) {
  const { t } = useTranslation();
  const navigate = useRouterStore((s) => s.navigate);
  const openHarness = useRuntimeStore((s) => s.openHarness);
  const instances = useRuntimeStore((s) => s.instances);
  const loadInstances = useRuntimeStore((s) => s.loadInstances);
  const overview = useRuntimeStore((s) => s.overview);
  const runtimeRestart = useRuntimeStore((s) => s.runtimeRestart);
  const loadProviders = useProviderStore((s) => s.loadProviders);
  const loadModels = useProviderStore((s) => s.loadModels);
  const providers = useProviderStore((s) => s.providers[profile.id]) ?? [];
  const models = useProviderStore((s) => s.models[profile.id]) ?? [];
  const plugins = usePluginStore((s) => s.plugins);
  const loadPlugins = usePluginStore((s) => s.loadPlugins);
  const manage = useScenarioManage();
  const busy = manage.busy;

  const [copiedUrl, setCopiedUrl] = useState(false);

  useEffect(() => {
    void loadInstances();
    void loadProviders(profile.id);
    void loadModels(profile.id);
    void loadPlugins();
  }, [loadInstances, loadProviders, loadModels, loadPlugins, profile.id]);

  const instance = instances.find((item) => item.profile === profile.id);
  const surface = inferSurface(profile, instance?.surface ?? "undetermined");
  const running = instance?.status === "running";
  const isDefault = isDefaultScenario(profile);
  const { blurb } = scenarioLayers(profile.description);

  const scenarioPlugins = plugins.filter((p) => p.profile === profile.id);
  const activePlugins = scenarioPlugins.filter((p) => p.enabled);
  const outdatedPlugins = scenarioPlugins.filter((p) => p.outdated);

  const subtitle = blurb
    ? blurb
    : running
      ? (instance?.url ?? t("overview.scenarioReady"))
      : surface === "task"
        ? t("overview.taskHint")
        : t("scene.stoppedHint");

  const copyUrl = async () => {
    if (!instance?.url) return;
    await navigator.clipboard.writeText(instance.url);
    setCopiedUrl(true);
    setTimeout(() => setCopiedUrl(false), 1500);
  };

  return (
    <PageBody className="space-y-5">
      <EngineMissingBanner />

      {/* Hero Scenario Card */}
      <Card>
        <CardContent className="p-5">
          <div className="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
            <div className="flex min-w-0 items-start gap-3.5">
              <div
                className={cn(
                  "flex h-14 w-14 shrink-0 items-center justify-center rounded-2xl text-title font-bold shadow-sm transition-colors",
                  running ? "bg-accent text-on-accent" : "bg-panel-2 text-text",
                )}
              >
                {scenarioInitial(profile.name)}
              </div>
              <div className="min-w-0">
                <div className="flex flex-wrap items-center gap-2">
                  <h1 className="text-display font-bold text-text">{profile.name}</h1>
                  {isDefault && (
                    <Badge variant="neutral" dot={false}>
                      {t("scene.default")}
                    </Badge>
                  )}
                  <SurfaceBadge surface={surface} />
                  <Badge variant={running ? "pass" : "neutral"}>
                    {running ? t("settings.runningBadge") : t("settings.scenarioStopped")}
                  </Badge>
                </div>
                <p className="mt-1 text-small text-text-dim">{subtitle}</p>
                {instance?.pid != null && (
                  <p className="mt-1 font-mono text-caption text-text-faint">
                    pid {instance.pid}
                    {instance.port != null ? ` · :${instance.port}` : ""}
                  </p>
                )}
              </div>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              {surface !== "task" && (
                <WebRuntimeControls profileId={profile.id} running={running} pid={instance?.pid} />
              )}
              <Button
                variant="ghost"
                size="icon"
                onClick={() => manage.open("description", profile)}
                title={t("settings.editDescription")}
                aria-label={t("settings.editDescription")}
              >
                <FileText className="h-4 w-4" />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                onClick={() => manage.open("rename", profile)}
                disabled={isDefault || busy}
                title={isDefault ? t("settings.protectedProfile") : t("settings.rename")}
                aria-label={t("settings.rename")}
              >
                <SquarePen className="h-4 w-4" />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                onClick={() => manage.open("delete", profile)}
                disabled={isDefault || busy}
                title={isDefault ? t("settings.protectedProfile") : t("settings.remove")}
                aria-label={t("settings.remove")}
              >
                <Trash2 className="h-4 w-4" />
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Restart Notification Banner if changes made */}
      {running && surface === "web" && (
        <Card className="border-accent/40 bg-accent-soft/30">
          <CardContent className="flex flex-col gap-2 p-3.5 md:flex-row md:items-center md:justify-between">
            <span className="text-small text-text-dim">{t("settings.restartToApply")}</span>
            <Button size="sm" onClick={() => runtimeRestart(profile.id)} disabled={busy}>
              <RotateCw className="h-4 w-4" />
              {t("overview.restart")}
            </Button>
          </CardContent>
        </Card>
      )}

      {/* Dashboard Metrics Grid */}
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
        {/* Metric 1: Runtime & Network */}
        <Card interactive className="flex flex-col justify-between">
          <CardContent className="p-4">
            <div className="flex items-center justify-between">
              <span className="flex items-center gap-2 text-small font-medium text-text-dim">
                <Activity className="h-4 w-4 text-text-faint" />
                {t("overview.metricRuntime")}
              </span>
              <Badge variant={running ? "pass" : "neutral"}>
                {running ? t("settings.runningBadge") : t("settings.scenarioStopped")}
              </Badge>
            </div>

            <div className="mt-3">
              {running ? (
                <div className="space-y-1.5">
                  <div className="font-mono text-heading font-semibold text-text">
                    {instance?.url ?? `http://127.0.0.1:${instance?.port ?? 3080}`}
                  </div>
                  <div className="flex flex-wrap items-center gap-2 pt-1">
                    <Button variant="secondary" size="sm" onClick={() => openHarness(profile.id)}>
                      <ExternalLink className="h-3.5 w-3.5" />
                      {t("overview.openConsole")}
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={copyUrl}
                      title={copiedUrl ? t("overview.copiedUrl") : t("overview.copyUrl")}
                    >
                      {copiedUrl ? (
                        <Check className="h-3.5 w-3.5 text-pass" />
                      ) : (
                        <Copy className="h-3.5 w-3.5" />
                      )}
                      {copiedUrl ? t("overview.copiedUrl") : t("overview.copyUrl")}
                    </Button>
                  </div>
                </div>
              ) : (
                <p className="text-small text-text-dim">
                  {surface === "web" ? t("settings.surfaceWebHint") : t("settings.surfaceTaskHint")}
                </p>
              )}
            </div>
          </CardContent>
        </Card>

        {/* Metric 2: Models & Providers */}
        <Card
          interactive
          className="flex flex-col justify-between"
          onClick={() => navigate({ kind: "scenario", section: "providers" })}
        >
          <CardContent className="p-4">
            <div className="flex items-center justify-between">
              <span className="flex items-center gap-2 text-small font-medium text-text-dim">
                <Cloud className="h-4 w-4 text-text-faint" />
                {t("overview.metricModels")}
              </span>
              <ArrowUpRight className="h-4 w-4 text-text-faint" />
            </div>
            <div className="mt-3">
              <div className="text-display font-bold text-text">{models.length}</div>
              <p className="mt-0.5 text-small text-text-dim">
                {t("overview.providersCount", { count: providers.length })}
              </p>
            </div>
            <div className="mt-3 flex items-center gap-1 text-small font-medium text-accent">
              {t("overview.manageModels")}
              <ArrowUpRight className="h-3.5 w-3.5" />
            </div>
          </CardContent>
        </Card>

        {/* Metric 3: Plugins & Extensions */}
        <Card
          interactive
          className="flex flex-col justify-between"
          onClick={() => navigate({ kind: "scenario", section: "plugins" })}
        >
          <CardContent className="p-4">
            <div className="flex items-center justify-between">
              <span className="flex items-center gap-2 text-small font-medium text-text-dim">
                <Puzzle className="h-4 w-4 text-text-faint" />
                {t("overview.metricPlugins")}
              </span>
              <ArrowUpRight className="h-4 w-4 text-text-faint" />
            </div>
            <div className="mt-3">
              <div className="flex items-baseline gap-2">
                <span className="text-display font-bold text-text">{scenarioPlugins.length}</span>
                <span className="text-caption text-text-faint">
                  ({t("overview.pluginsActive", { count: activePlugins.length })})
                </span>
              </div>
              <p className="mt-0.5 text-small text-text-dim">
                {outdatedPlugins.length > 0 ? (
                  <span className="text-warn font-medium">
                    {t("overview.pluginsOutdatedAlert", { count: outdatedPlugins.length })}
                  </span>
                ) : (
                  t("settings.scenarioPluginCount", { count: scenarioPlugins.length })
                )}
              </p>
            </div>
            <div className="mt-3 flex items-center gap-1 text-small font-medium text-accent">
              {t("overview.managePlugins")}
              <ArrowUpRight className="h-3.5 w-3.5" />
            </div>
          </CardContent>
        </Card>

        {/* Metric 4: AI Engine Health */}
        <Card
          interactive
          className="flex flex-col justify-between"
          onClick={() => navigate({ kind: "settings", section: "diagnostics" })}
        >
          <CardContent className="p-4">
            <div className="flex items-center justify-between">
              <span className="flex items-center gap-2 text-small font-medium text-text-dim">
                <ShieldCheck className="h-4 w-4 text-text-faint" />
                {t("overview.metricEngine")}
              </span>
              <ArrowUpRight className="h-4 w-4 text-text-faint" />
            </div>
            <div className="mt-3">
              <div className="truncate text-heading font-semibold text-text">
                {overview?.detection.harness?.name ?? "DeepSeek Harness"}
              </div>
              <p className="mt-0.5 text-small text-text-dim">
                {overview?.detection.harness?.version
                  ? `v${overview.detection.harness.version}`
                  : t("overview.engineReady")}
              </p>
            </div>
            <div className="mt-3 flex items-center gap-1 text-small font-medium text-accent">
              {t("overview.runDoctor")}
              <ArrowUpRight className="h-3.5 w-3.5" />
            </div>
          </CardContent>
        </Card>
      </div>

      {surface === "task" && <TaskRunner profileId={profile.id} />}

      <ScenarioManageDialogs manage={manage} />
    </PageBody>
  );
}
