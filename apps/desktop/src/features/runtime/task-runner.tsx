// One-shot task runner for task-surface scenarios. The run state lives in
// the runtime store keyed by scenario, so switching scenarios mid-run keeps
// the stream and its outcome.

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { AlertTriangle, CheckCircle2, Loader2, TerminalSquare } from "lucide-react";
import { useRuntimeStore } from "@/app/store/runtime";
import { Card, CardContent } from "@/shared/ui/card";
import { Button } from "@/shared/ui/button";
import { Input } from "@/shared/ui/input";

export function TaskRunner({ profileId }: { profileId: string }) {
  const { t } = useTranslation();
  const runTask = useRuntimeStore((s) => s.runTask);
  const task = useRuntimeStore((s) => s.tasks[profileId]) ?? IDLE;
  const [prompt, setPrompt] = useState("");

  const run = () => {
    const trimmed = prompt.trim();
    if (!trimmed || task.running) return;
    void runTask(profileId, trimmed);
  };

  return (
    <section className="space-y-3">
      <h2 className="text-heading font-semibold text-text">{t("settings.runTask")}</h2>
      <Card>
        <CardContent className="flex flex-col gap-2 p-4 md:flex-row">
          <Input
            value={prompt}
            onChange={(event) => setPrompt(event.target.value)}
            placeholder={t("settings.taskPrompt")}
            className="flex-1"
            disabled={task.running}
            onKeyDown={(event) => {
              if (event.key === "Enter") run();
            }}
          />
          <Button
            variant="primary"
            onClick={run}
            disabled={task.running || !prompt.trim()}
          >
            {task.running ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <TerminalSquare className="h-4 w-4" />
            )}
            {task.running ? t("settings.taskRunning") : t("settings.runTask")}
          </Button>
        </CardContent>
      </Card>
      {task.lines.length > 0 && (
        <Card>
          <CardContent className="flex flex-col gap-2 p-4">
            <div className="flex items-center gap-2">
              {task.failed ? (
                <AlertTriangle className="h-4 w-4 shrink-0 text-fail" />
              ) : task.done ? (
                <CheckCircle2 className="h-4 w-4 shrink-0 text-pass" />
              ) : (
                <Loader2 className="h-4 w-4 shrink-0 animate-spin text-accent" />
              )}
              <span className="text-small font-medium text-text">
                {task.failed
                  ? t("settings.taskFailed")
                  : task.done
                    ? t("settings.taskSuccess")
                    : t("settings.taskRunning")}
              </span>
            </div>
            <div className="max-h-72 overflow-y-auto rounded-md border border-border bg-inset p-3 font-mono text-caption text-text-dim">
              {task.lines.map((line, index) => (
                <div key={index} className="whitespace-pre-wrap break-words">
                  {line}
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      )}
    </section>
  );
}

const IDLE = { running: false, done: false, failed: false, lines: [] };
