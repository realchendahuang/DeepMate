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
} from "lucide-react";
import { useStore } from "../store";
import { Button } from "../components/ui/button";
import { Card, CardContent } from "../components/ui/card";
import { Badge } from "../components/ui/badge";
import { EmptyState } from "../components/ui/data-list";
import { PageBody, PageHeader, SectionHeader } from "../components/ui/page";
import type { CheckStatus } from "../types";
import { cn } from "../lib/utils";

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

export function OverviewPage() {
  const { t } = useTranslation();
  const overview = useStore((s) => s.overview);
  const doctor = useStore((s) => s.doctor);
  const busy = useStore((s) => s.busy);
  const refreshAll = useStore((s) => s.refreshAll);
  const runtimeStart = useStore((s) => s.runtimeStart);
  const runtimeStop = useStore((s) => s.runtimeStop);
  const runtimeRestart = useStore((s) => s.runtimeRestart);
  const openHarness = useStore((s) => s.openHarness);
  const runDoctor = useStore((s) => s.runDoctor);
  const updateInfo = useStore((s) => s.updateInfo);
  const openRelease = useStore((s) => s.openRelease);

  useEffect(() => {
    refreshAll();
  }, [refreshAll]);

  if (!overview) {
    return <PageBody className="text-text-dim">{t("overview.title")}...</PageBody>;
  }

  const { detection, status, counts } = overview;
  const harnessFound = detection.found;
  const statusTone = STATUS_TONE[status.kind] ?? "neutral";
  const running = status.kind === "running";
  const stats = [
    { label: t("overview.profiles"), value: counts.profiles, icon: Layers },
    { label: t("overview.providers"), value: counts.providers, icon: Cloud },
    { label: t("overview.models"), value: counts.models, icon: Cpu },
    { label: t("overview.plugins"), value: counts.plugins, icon: Package },
  ];

  return (
    <PageBody className="space-y-5">
      <PageHeader
        title={t("overview.title")}
        actions={
          <Button type="button" onClick={refreshAll} disabled={busy}>
            <RotateCw className="h-4 w-4" />
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
            <Button
              variant="primary"
              size="sm"
              onClick={() => openRelease(updateInfo.url)}
              title={updateInfo.url}
            >
              <ExternalLink className="h-4 w-4" />
              {t("overview.viewRelease")}
            </Button>
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
            <Button variant="primary" onClick={openHarness} disabled={busy || !running}>
              <ExternalLink className="h-4 w-4" />
              {t("overview.openHarness")}
            </Button>
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
        {stats.map(({ label, value, icon: Icon }) => (
          <Card key={label}>
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
        ))}
      </div>

      <section className="space-y-3">
        <SectionHeader
          title={t("overview.diagnostics")}
          actions={
            <Button variant="primary" onClick={runDoctor} disabled={busy}>
              <ShieldCheck className="h-4 w-4" />
              {busy ? t("overview.running") : t("overview.runDoctor")}
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
