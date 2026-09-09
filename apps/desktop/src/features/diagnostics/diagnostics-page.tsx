// The diagnostics page (Settings → 诊断): the doctor report with one-click
// repairs, plus the engine-install guidance when the harness CLI is missing.
// This is the only global health surface — scenarios own their runtime, this
// page owns the engine itself.

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  CheckCircle2,
  AlertTriangle,
  XCircle,
  MinusCircle,
  ShieldCheck,
  Check,
  Copy,
  RotateCw,
} from "lucide-react";
import { motion } from "motion/react";
import { useDiagnosticsStore } from "@/app/store/diagnostics";
import { useBusyStore } from "@/app/store/busy";
import type { CheckStatus, DoctorCheck } from "@/shared/api/api";
import type { TFunction } from "i18next";
import { cn } from "@/shared/lib/utils";
import { enterTransition } from "@/shared/lib/motion";
import { Card } from "@/shared/ui/card";
import { Button } from "@/shared/ui/button";
import { Badge } from "@/shared/ui/badge";
import { Skeleton } from "@/shared/ui/skeleton";
import { EmptyState } from "@/shared/ui/empty-state";
import { PageBody, SectionHeader } from "@/shared/ui/page";

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

// The doctor report ships English summary/details from the Rust side; the UI
// owns all copy. Every check renders as one always-visible row: a
// plain-language title, an optional right-aligned technical value (label +
// monospace chip), guidance and technical lines below. Nothing is hidden
// behind a collapse — the technical details stay on screen, just well set
// out. Unknown check ids fall back to the raw fields of the report.
type DoctorRow = {
  title: string;
  desc: string | null;
  meta: { label: string | null; value: string } | null;
  extraLines: string[];
  showInstall: boolean;
  rawAction: string | null;
  // One-click repairs offered for a failing check: the button label key and
  // the mode passed to doctor_fix ("clear" / "reinstall" / "restart" /
  // "start").
  fixes?: { mode: string; labelKey: string; danger?: boolean }[];
};

function doctorRow(check: DoctorCheck, t: TFunction): DoctorRow {
  if (check.id === "runtime.installed") {
    if (check.status === "pass") {
      return {
        title: t("doctor.runtimeFound"),
        desc: null,
        meta: check.cli ? { label: t("doctor.metaCommand"), value: check.cli } : null,
        extraLines: [],
        showInstall: false,
        rawAction: null,
      };
    }
    return {
      title: t("doctor.runtimeNotFound"),
      desc: t("doctor.engineDesc"),
      meta: null,
      extraLines: [t("doctor.runtimeNotFoundDetail")],
      showInstall: true,
      rawAction: null,
    };
  }
  if (check.id === "ui.url") {
    if (check.status === "pass") {
      return {
        title: t("doctor.uiUrlConfigured"),
        desc: null,
        meta: check.url ? { label: null, value: check.url } : null,
        extraLines: [],
        showInstall: false,
        rawAction: null,
      };
    }
    return {
      title: t("doctor.uiUrlInvalid"),
      desc: t("doctor.uiUrlInvalidDesc"),
      meta: check.url ? { label: null, value: check.url } : null,
      extraLines: [],
      showInstall: false,
      rawAction: null,
    };
  }
  if (check.id === "ui.reachable") {
    if (check.status === "skip") {
      return {
        title: t("doctor.uiReachSkipped"),
        desc: t("doctor.uiReachSkippedDesc"),
        meta: null,
        extraLines: [],
        showInstall: false,
        rawAction: null,
      };
    }
    if (check.status === "pass") {
      return {
        title: t("doctor.uiReachable"),
        desc: null,
        meta: check.url ? { label: null, value: check.url } : null,
        extraLines: [],
        showInstall: false,
        rawAction: null,
      };
    }
    return {
      title: t("doctor.uiNotRunning"),
      desc: t("doctor.consoleDesc"),
      meta: check.url ? { label: null, value: check.url } : null,
      extraLines: [],
      showInstall: false,
      rawAction: null,
      fixes: [{ mode: "start", labelKey: "doctor.fixStart" }],
    };
  }
  if (check.id === "plugins.bundles") {
    if (check.status === "pass") {
      return {
        title: t("doctor.bundlesOk"),
        desc: null,
        meta: null,
        extraLines: [],
        showInstall: false,
        rawAction: null,
      };
    }
    return {
      title: t("doctor.bundlesMissing"),
      desc: t("doctor.bundlesMissingDesc"),
      meta: null,
      extraLines: check.details ? [check.details] : [],
      showInstall: false,
      rawAction: t("doctor.bundlesAction"),
      fixes: [
        { mode: "reinstall", labelKey: "doctor.fixReinstall" },
        { mode: "clear", labelKey: "doctor.fixClear", danger: true },
      ],
    };
  }
  if (check.id === "plugins.bundles.load") {
    if (check.status === "skip") {
      return {
        title: t("doctor.bundlesLoadSkipped"),
        desc: null,
        meta: null,
        extraLines: [],
        showInstall: false,
        rawAction: null,
      };
    }
    if (check.status === "pass") {
      return {
        title: t("doctor.bundlesLoadOk"),
        desc: null,
        meta: null,
        extraLines: [],
        showInstall: false,
        rawAction: null,
      };
    }
    return {
      title: t("doctor.bundlesLoadFailed"),
      desc: t("doctor.bundlesLoadFailedDesc"),
      meta: null,
      extraLines: check.details ? [check.details] : [],
      showInstall: false,
      rawAction: t("doctor.bundlesLoadAction"),
      fixes: [
        // A running console keeps the bundle set it started with: restart is
        // the first remedy.
        { mode: "restart", labelKey: "doctor.fixRestart" },
        { mode: "reinstall", labelKey: "doctor.fixReinstall" },
      ],
    };
  }
  // Unknown check ids keep the raw report fields visible.
  return {
    title: check.summary,
    desc: check.details,
    meta: check.url ? { label: t("doctor.metaCommand"), value: check.url } : null,
    extraLines: [],
    showInstall: false,
    rawAction: check.suggested_action,
  };
}

export function DiagnosticsPage() {
  const { t } = useTranslation();
  const doctor = useDiagnosticsStore((s) => s.doctor);
  const runDoctor = useDiagnosticsStore((s) => s.runDoctor);
  const fixDoctor = useDiagnosticsStore((s) => s.fixDoctor);
  const busyAction = useBusyStore((s) => s.busyAction);

  const [copied, setCopied] = useState(false);

  // Fresh diagnostics on every visit; the previous report renders in between.
  useEffect(() => {
    void runDoctor();
  }, [runDoctor]);

  const busy = busyAction !== null;
  const activeChecks = doctor?.checks.filter((check) => check.status !== "skip") ?? [];
  const failingChecks = activeChecks.filter((check) => check.status !== "pass");
  const engineMissing = doctor?.checks.find((check) => check.id === "runtime.installed")
    ?.status === "fail";

  const copyInstallCommand = async () => {
    try {
      await navigator.clipboard.writeText(t("doctor.installCommand"));
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2000);
    } catch {
      // Clipboard unavailable: leaving the button label unchanged is enough.
    }
  };

  return (
    <PageBody className="space-y-5">
      <SectionHeader
        title={t("settings.diagnostics")}
        actions={
          <Button variant="primary" onClick={runDoctor} disabled={busy}>
            <RotateCw className={cn("h-4 w-4", busyAction === "doctor" && "animate-spin")} />
            {busyAction === "doctor" ? t("overview.running") : t("overview.runDoctor")}
          </Button>
        }
      />

      {!doctor ? (
        <Skeleton className="h-32 w-full" />
      ) : doctor.checks.length === 0 ? (
        <EmptyState icon={<ShieldCheck className="h-8 w-8 text-text-faint" />}>
          {t("overview.runDoctorEmpty")}
        </EmptyState>
      ) : (
        <motion.div
          initial={{ opacity: 0, y: 4 }}
          animate={{ opacity: 1, y: 0 }}
          transition={enterTransition}
          className="space-y-4"
        >
          {/* Missing engine alert */}
          {engineMissing && (
            <Card className="border-warn/40 bg-warn/5">
              <div className="flex flex-col gap-3 p-4 md:flex-row md:items-center md:justify-between">
                <div>
                  <div className="text-heading font-semibold text-text">
                    {t("overview.notDetected")}
                  </div>
                  <p className="mt-0.5 text-small text-text-dim">{t("overview.notDetectedHint")}</p>
                </div>
                <div className="flex items-center gap-2">
                  <code className="rounded border border-border bg-panel-2 px-2.5 py-1 font-mono text-caption text-text">
                    {t("doctor.installCommand")}
                  </code>
                  <Button variant="secondary" size="sm" onClick={copyInstallCommand}>
                    {copied ? <Check className="h-3.5 w-3.5 text-pass" /> : <Copy className="h-3.5 w-3.5" />}
                    {copied ? t("doctor.copied") : t("doctor.copyCommand")}
                  </Button>
                </div>
              </div>
            </Card>
          )}

          {/* System Health Status Banner */}
          <Card
            className={cn(
              "flex flex-col gap-3 p-4 sm:flex-row sm:items-center sm:justify-between",
              failingChecks.length === 0
                ? "border-pass/30 bg-pass/5"
                : "border-warn/30 bg-warn/5",
            )}
          >
            <div className="flex items-center gap-3">
              <div
                className={cn(
                  "flex h-10 w-10 shrink-0 items-center justify-center rounded-xl",
                  failingChecks.length === 0
                    ? "bg-pass/15 text-pass"
                    : "bg-warn/15 text-warn",
                )}
              >
                {failingChecks.length === 0 ? (
                  <CheckCircle2 className="h-5 w-5" />
                ) : (
                  <AlertTriangle className="h-5 w-5" />
                )}
              </div>
              <div>
                <h3 className="text-heading font-semibold text-text">
                  {failingChecks.length === 0
                    ? t("settings.healthAllGood")
                    : t("settings.healthHasIssues", { count: failingChecks.length })}
                </h3>
                <p className="text-small text-text-dim">
                  {failingChecks.length === 0
                    ? t("doctor.allGoodLine", { count: activeChecks.length })
                    : t("doctor.issuesLine", { count: failingChecks.length })}
                </p>
              </div>
            </div>

            <div className="flex items-center gap-2">
              <Badge variant={failingChecks.length === 0 ? "pass" : "warn"}>
                {failingChecks.length === 0
                  ? `${activeChecks.length}/${activeChecks.length} Passed`
                  : `${failingChecks.length} Issues`}
              </Badge>
            </div>
          </Card>

          {/* Check Item Cards */}
          <Card className="overflow-hidden">
            <div className="divide-y divide-border">
              {doctor.checks.map((check) => {
                const row = doctorRow(check, t);
                const Icon = CHECK_ICON[check.status];
                return (
                  <div
                    key={check.id}
                    className="flex gap-3 p-4 transition-colors hover:bg-hover/20"
                  >
                    <Icon
                      className={cn(
                        "mt-0.5 h-5 w-5 shrink-0",
                        CHECK_TONE[check.status],
                      )}
                    />
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
                        <span className="text-body font-medium text-text">{row.title}</span>
                        {row.meta && (
                          <span className="flex shrink-0 items-center gap-1.5">
                            {row.meta.label && (
                              <span className="text-caption text-text-faint">
                                {row.meta.label}
                              </span>
                            )}
                            <code className="rounded border border-border bg-panel-2 px-1.5 py-0.5 font-mono text-caption text-text-dim">
                              {row.meta.value}
                            </code>
                          </span>
                        )}
                      </div>
                      {row.desc && (
                        <p className="mt-0.5 text-small text-text-dim">{row.desc}</p>
                      )}
                      {row.extraLines.map((line) => (
                        <p key={line} className="mt-0.5 text-caption text-text-faint">
                          {line}
                        </p>
                      ))}
                      {row.showInstall && (
                        <div className="mt-2 flex flex-wrap items-center gap-2">
                          <code className="rounded border border-border bg-panel-2 px-2 py-1 font-mono text-caption text-text-dim">
                            {t("doctor.installCommand")}
                          </code>
                          <Button variant="secondary" size="sm" onClick={copyInstallCommand}>
                            {copied ? <Check className="h-3.5 w-3.5 text-pass" /> : <Copy className="h-3.5 w-3.5" />}
                            {copied ? t("doctor.copied") : t("doctor.copyCommand")}
                          </Button>
                        </div>
                      )}
                      {row.rawAction && (
                        <p className="mt-1 text-small text-accent">
                          {t("overview.suggestedAction", { action: row.rawAction })}
                        </p>
                      )}
                      {row.fixes && row.fixes.length > 0 && (
                        <div className="mt-2.5 flex flex-wrap items-center gap-2">
                          {row.fixes.map((fix) => (
                            <Button
                              key={fix.mode}
                              variant={fix.danger ? "danger" : "primary"}
                              size="sm"
                              disabled={busy}
                              onClick={() => fixDoctor(check.id, fix.mode)}
                            >
                              {t(fix.labelKey)}
                            </Button>
                          ))}
                        </div>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          </Card>
        </motion.div>
      )}
    </PageBody>
  );
}
