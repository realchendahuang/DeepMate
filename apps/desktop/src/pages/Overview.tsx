import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import {
  ExternalLink,
  Play,
  Square,
  RotateCw,
  ShieldCheck,
  Layers,
  Cloud,
  Cpu,
  Package,
  CheckCircle2,
  AlertTriangle,
  XCircle,
  MinusCircle,
  Sparkles,
  Download,
  X,
} from "lucide-react";
import { useStore } from "../store";
import { Button } from "../components/ui/button";
import { Card, CardContent } from "../components/ui/card";
import { Badge } from "../components/ui/badge";
import { Skeleton } from "../components/ui/skeleton";
import { EmptyState } from "../components/ui/empty-state";
import { PageBody, PageHeader, SectionHeader } from "../components/ui/page";
import { Tooltip, TooltipContent, TooltipTrigger } from "../components/ui/tooltip";
import type { CheckStatus } from "../api";
import { cn } from "../lib/utils";
import type { View } from "../components/layout/nav";

const STATUS_TONE: Record<string, string> = {
  running: "pass",
  installed: "accent",
  stopped: "neutral",
  unknown: "skip",
  error: "fail",
};

const CHECK_ICON: Record<CheckStatus, typeof CheckCircle2> = {
  pass: CheckCircle2,
  warn: AlertTriangle,
  fail: XCircle,
  skip: MinusCircle,
};

const CHECK_TONE: Record<CheckStatus, string> = {
  pass: "text-pass",
  warn: "text-warn",
  fail: "text-fail",
  skip: "text-skip",
};

export function OverviewPage({ onNavigate }: { onNavigate?: (view: View) => void }) {
  const { t } = useTranslation();
  const overview = useStore((s) => s.overview);
  const doctor = useStore((s) => s.doctor);
  const busyAction = useStore((s) => s.busyAction);
  const refreshAll = useStore((s) => s.refreshAll);
  const runtimeStart = useStore((s) => s.runtimeStart);
  const runtimeStop = useStore((s) => s.runtimeStop);
  const runtimeRestart = useStore((s) => s.runtimeRestart);
  const openHarness = useStore((s) => s.openHarness);
  const runDoctor = useStore((s) => s.runDoctor);
  const updateInfo = useStore((s) => s.updateInfo);
  const installUpdate = useStore((s) => s.installUpdate);
  const openRelease = useStore((s) => s.openRelease);
  const dismissUpdate = useStore((s) => s.dismissUpdate);

  useEffect(() => {
    refreshAll();
  }, [refreshAll]);

  if (!overview) {
    return (
      <PageBody className="space-y-5">
        <PageHeader title={t("overview.title")} />
        <Skeleton className="h-40 w-full" />
        <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
          {[0, 1, 2, 3].map((i) => (
            <Skeleton key={i} className="h-20 w-full" />
          ))}
        </div>
        <Skeleton className="h-32 w-full" />
      </PageBody>
    );
  }

  const { detection, status, counts } = overview;
  const harnessFound = detection.found;
  const statusTone = STATUS_TONE[status.kind] ?? "neutral";
  const running = status.kind === "running";
  const refreshing = busyAction === "refresh";
  const installing = busyAction === "update-install";
  const busy = busyAction !== null;

  const stats = [
    { label: t("overview.profiles"), value: counts.profiles, icon: Layers, view: "settings" as View },
    { label: t("overview.providers"), value: counts.providers, icon: Cloud, view: "settings" as View },
    { label: t("overview.models"), value: counts.models, icon: Cpu, view: "settings" as View },
    { label: t("overview.plugins"), value: counts.plugins, icon: Package, view: "plugins" as View },
  ];

  return (
    <PageBody className="space-y-5">
      <PageHeader
        title={t("overview.title")}
        actions={
          <Button type="button" onClick={refreshAll} disabled={refreshing}>
            <RotateCw className={cn("h-4 w-4", refreshing && "animate-spin")} />
            {t("overview.refresh")}
          </Button>
        }
      />

      {updateInfo && (
        <Card className="border-accent/40">
          <CardContent className="flex flex-col gap-2 p-4 md:flex-row md:items-center md:justify-between">
            <div className="flex items-center gap-2 text-body text-text">
              <Sparkles className="h-4 w-4 shrink-0 text-accent" />
              {t("overview.updateAvailable", { version: updateInfo.latest_version })}
            </div>
            <div className="flex flex-wrap gap-2">
              <Button
                variant="primary"
                size="sm"
                onClick={installUpdate}
                disabled={installing}
                title={t("overview.installUpdateHint")}
              >
                <Download className={cn("h-4 w-4", installing && "animate-pulse")} />
                {installing ? t("overview.downloading") : t("overview.installUpdate")}
              </Button>
              <Button variant="secondary" size="sm" onClick={() => openRelease(updateInfo.url)} title={updateInfo.url}>
                <ExternalLink className="h-4 w-4" />
                {t("overview.viewRelease")}
              </Button>
              <Button variant="ghost" size="sm" onClick={dismissUpdate} aria-label={t("overview.dismissUpdate")}>
                <X className="h-4 w-4" />
                {t("overview.dismissUpdate")}
              </Button>
            </div>
          </CardContent>
        </Card>
      )}

      <Card>
        <CardContent className="p-4 md:p-5">
          <div className="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
            <div className="min-w-0 space-y-2">
              <div className="flex flex-wrap items-center gap-2">
                <h2 className="text-display font-bold text-text">
                  {harnessFound ? detection.harness?.name : t("overview.notDetected")}
                </h2>
                <Badge variant={harnessFound ? "pass" : "warn"}>
                  {harnessFound ? t("overview.detected") : t("overview.missing")}
                </Badge>
                {detection.harness?.version && (
                  <span className="text-small text-text-dim">
                    {t("overview.version", { version: detection.harness.version })}
                  </span>
                )}
              </div>
              {detection.detail && <p className="text-small text-text-faint">{detection.detail}</p>}
              <div className="flex flex-wrap items-center gap-2">
                <span
                  className={cn("h-2.5 w-2.5 rounded-full", {
                    "bg-pass": statusTone === "pass",
                    "bg-accent": statusTone === "accent",
                    "bg-neutral": statusTone === "neutral",
                    "bg-skip": statusTone === "skip",
                    "bg-fail": statusTone === "fail",
                  })}
                />
                <span className="text-body font-semibold text-text">{t(`status.${status.kind}`)}</span>
                {status.pid != null && (
                  <span className="text-small text-text-faint">{t("overview.pid", { pid: status.pid })}</span>
                )}
              </div>
            </div>
            <Tooltip>
              <TooltipTrigger asChild>
                <span>
                  <Button variant="primary" onClick={openHarness} disabled={busy || !running}>
                    <ExternalLink className="h-4 w-4" />
                    {t("overview.openHarness")}
                  </Button>
                </span>
              </TooltipTrigger>
              <TooltipContent>{t("overview.openHarnessHint")}</TooltipContent>
            </Tooltip>
          </div>

          {harnessFound && detection.harness && (
            <div className="mt-4 flex flex-wrap gap-1.5 border-t border-border pt-4">
              {["runtime", "profiles", "providers", "models", "plugins", "marketplace"].map((cap) => (
                <Badge key={cap} variant="accent" dot={false}>
                  {cap}
                </Badge>
              ))}
            </div>
          )}

          <div className="mt-4 flex flex-wrap items-center gap-2 border-t border-border pt-4">
            <div className="hidden flex-1 md:block" />
            <Button onClick={runtimeStart} disabled={busy || running}>
              <Play className="h-4 w-4" />
              {t("overview.start")}
            </Button>
            <Button onClick={runtimeStop} disabled={busy || status.pid == null}>
              <Square className="h-4 w-4" />
              {t("overview.stop")}
            </Button>
            <Button onClick={runtimeRestart} disabled={busy || status.pid == null}>
              <RotateCw className="h-4 w-4" />
              {t("overview.restart")}
            </Button>
          </div>
        </CardContent>
      </Card>

      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        {stats.map(({ label, value, icon: Icon, view: target }) => (
          <button
            key={label}
            type="button"
            onClick={() => onNavigate?.(target)}
            className="rounded-lg text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
          >
            <Card className="transition-colors hover:bg-hover">
              <CardContent className="flex items-center gap-3 p-4">
                <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-md bg-accent-soft">
                  <Icon className="h-[18px] w-[18px] text-accent" />
                </div>
                <div className="min-w-0">
                  <div className="text-display font-bold text-text">{value == null ? "-" : value}</div>
                  <div className="truncate text-small text-text-dim">{label}</div>
                </div>
              </CardContent>
            </Card>
          </button>
        ))}
      </div>

      <section className="space-y-3">
        <SectionHeader
          title={t("overview.diagnostics")}
          actions={
            <Button variant="primary" onClick={runDoctor} disabled={busyAction === "doctor"}>
              <ShieldCheck className="h-4 w-4" />
              {busyAction === "doctor" ? t("overview.running") : t("overview.runDoctor")}
            </Button>
          }
        />

        {!doctor || doctor.checks.length === 0 ? (
          <EmptyState icon={<ShieldCheck className="h-8 w-8 text-text-faint" />}>
            {t("overview.runDoctorEmpty")}
          </EmptyState>
        ) : (
          <Card>
            <div className="divide-y divide-border px-4 md:px-5">
              {doctor.checks.map((check) => {
                const Icon = CHECK_ICON[check.status];
                return (
                  <div key={check.id} className="flex gap-3 py-3">
                    <Icon className={cn("mt-0.5 h-[18px] w-[18px] shrink-0", CHECK_TONE[check.status])} />
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
                        <span className="text-body font-semibold text-text">{check.summary}</span>
                        <span className={cn("text-caption font-bold", CHECK_TONE[check.status])}>
                          {t(`check.${check.status}`)}
                        </span>
                      </div>
                      {check.details && <p className="text-small text-text-dim">{check.details}</p>}
                      {check.suggested_action && (
                        <p className="text-small text-accent">
                          {t("overview.suggestedAction", { action: check.suggested_action })}
                        </p>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          </Card>
        )}
      </section>
    </PageBody>
  );
}
