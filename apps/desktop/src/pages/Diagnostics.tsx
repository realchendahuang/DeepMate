// The diagnostics page (Settings → 诊断): the doctor report with one-click
// repairs, plus the engine-install guidance when the harness CLI is missing.
// This is the only global health surface — scenarios own their runtime, this
// page owns the engine itself.

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { CheckCircle2, AlertTriangle, XCircle, MinusCircle, ShieldCheck } from "lucide-react";
import { useStore } from "../store";
import { Card } from "../components/ui/card";
import { Button } from "../components/ui/button";
import { Skeleton } from "../components/ui/skeleton";
import { EmptyState } from "../components/ui/empty-state";
import { PageBody, SectionHeader } from "../components/ui/page";
import { cn } from "../lib/utils";
import type { CheckStatus, DoctorCheck } from "../api";
import type { TFunction } from "i18next";

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

export function DiagnosticsView() {
  const { t } = useTranslation();
  const doctor = useStore((s) => s.doctor);
  const busyAction = useStore((s) => s.busyAction);
  const runDoctor = useStore((s) => s.runDoctor);
  const fixDoctor = useStore((s) => s.fixDoctor);

  const [copied, setCopied] = useState(false);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    // Fresh diagnostics on every visit.
    runDoctor().finally(() => setLoaded(true));
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
            <ShieldCheck className="h-4 w-4" />
            {busyAction === "doctor" ? t("overview.running") : t("overview.runDoctor")}
          </Button>
        }
      />

      {!loaded ? (
        <Skeleton className="h-32 w-full" />
      ) : !doctor || doctor.checks.length === 0 ? (
        <EmptyState icon={<ShieldCheck className="h-8 w-8 text-text-faint" />}>
          {t("overview.runDoctorEmpty")}
        </EmptyState>
      ) : (
        <div className="space-y-3">
          {engineMissing && (
            <Card className="border-warn/40">
              <div className="flex flex-col gap-2 p-4 md:flex-row md:items-center md:justify-between">
                <div>
                  <div className="text-heading font-semibold text-text">
                    {t("overview.notDetected")}
                  </div>
                  <p className="mt-0.5 text-small text-text-dim">{t("overview.notDetectedHint")}</p>
                </div>
                <code className="rounded border border-border bg-panel-2 px-2 py-1 font-mono text-caption text-text-dim">
                  {t("doctor.installCommand")}
                </code>
              </div>
            </Card>
          )}

          {failingChecks.length === 0 ? (
            <p className="flex items-center gap-1.5 text-small text-pass">
              <CheckCircle2 className="h-4 w-4 shrink-0" />
              {t("doctor.allGoodLine", { count: activeChecks.length })}
            </p>
          ) : (
            <p className="flex items-center gap-1.5 text-small text-warn">
              <AlertTriangle className="h-4 w-4 shrink-0" />
              {t("doctor.issuesLine", { count: failingChecks.length })}
            </p>
          )}

          <Card>
            <div className="divide-y divide-border px-4 md:px-5">
              {doctor.checks.map((check) => {
                const row = doctorRow(check, t);
                const Icon = CHECK_ICON[check.status];
                return (
                  <div key={check.id} className="flex gap-3 py-3">
                    <Icon
                      className={cn(
                        "mt-0.5 h-[18px] w-[18px] shrink-0",
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
                        <div className="mt-1.5 flex items-center gap-1.5">
                          <code className="rounded border border-border bg-panel-2 px-1.5 py-0.5 font-mono text-caption text-text-dim">
                            {t("doctor.installCommand")}
                          </code>
                          <Button variant="secondary" size="sm" onClick={copyInstallCommand}>
                            {copied ? t("doctor.copied") : t("doctor.copyCommand")}
                          </Button>
                        </div>
                      )}
                      {row.rawAction && (
                        <p className="mt-0.5 text-small text-accent">
                          {t("overview.suggestedAction", { action: row.rawAction })}
                        </p>
                      )}
                      {row.fixes && row.fixes.length > 0 && (
                        <div className="mt-2 flex flex-wrap items-center gap-2">
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
        </div>
      )}
    </PageBody>
  );
}