// Compact operation status strip for a scenario's plugin ops: streams the
// newest harness line, keeps failures open with the full log.

import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { AlertTriangle, CheckCircle2, Loader2 } from "lucide-react";
import type { PluginOpEvent } from "@/shared/api/api";
import { Card, CardContent } from "@/shared/ui/card";
import { Button } from "@/shared/ui/button";

export function OpStatus({
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
