// One-shot task runner for task-surface scenarios. The run state lives in
// the runtime store keyed by scenario, so switching scenarios mid-run keeps
// the stream and its outcome.

import { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  AlertTriangle,
  Check,
  CheckCircle2,
  Copy,
  Loader2,
  TerminalSquare,
  Trash2,
} from "lucide-react";
import { useRuntimeStore } from "@/app/store/runtime";
import { Card, CardContent } from "@/shared/ui/card";
import { Button } from "@/shared/ui/button";
import { Input } from "@/shared/ui/input";

export function TaskRunner({ profileId }: { profileId: string }) {
  const { t } = useTranslation();
  const runTask = useRuntimeStore((s) => s.runTask);
  const task = useRuntimeStore((s) => s.tasks[profileId]) ?? IDLE;
  const [prompt, setPrompt] = useState("");
  const [copied, setCopied] = useState(false);

  const run = () => {
    const trimmed = prompt.trim();
    if (!trimmed || task.running) return;
    void runTask(profileId, trimmed);
  };

  const copyLogs = async () => {
    if (task.lines.length === 0) return;
    await navigator.clipboard.writeText(task.lines.join("\n"));
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  const clearLogs = () => {
    useRuntimeStore.setState((state) => ({
      tasks: {
        ...state.tasks,
        [profileId]: IDLE,
      },
    }));
  };

  return (
    <section className="space-y-3">
      <div className="flex items-center justify-between">
        <h2 className="text-heading font-semibold text-text">{t("settings.runTask")}</h2>
      </div>
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
        <Card className="overflow-hidden">
          <div className="flex items-center justify-between border-b border-border bg-panel-2/50 px-4 py-2.5">
            <div className="flex items-center gap-2">
              <div className="flex items-center gap-1.5" aria-hidden>
                <span className="h-2.5 w-2.5 rounded-full bg-border" />
                <span className="h-2.5 w-2.5 rounded-full bg-border" />
                <span className="h-2.5 w-2.5 rounded-full bg-border" />
              </div>
              <div className="ml-2 flex items-center gap-1.5">
                {task.failed ? (
                  <AlertTriangle className="h-3.5 w-3.5 shrink-0 text-fail" />
                ) : task.done ? (
                  <CheckCircle2 className="h-3.5 w-3.5 shrink-0 text-pass" />
                ) : (
                  <Loader2 className="h-3.5 w-3.5 shrink-0 animate-spin text-accent" />
                )}
                <span className="text-caption font-semibold text-text">
                  {task.failed
                    ? t("settings.taskFailed")
                    : task.done
                      ? t("settings.taskSuccess")
                      : t("settings.taskRunning")}
                </span>
              </div>
            </div>
            <div className="flex items-center gap-1">
              <Button
                variant="ghost"
                size="icon"
                className="h-7 w-7"
                onClick={copyLogs}
                title={copied ? t("overview.taskLogsCopied") : t("overview.taskCopyLogs")}
              >
                {copied ? (
                  <Check className="h-3.5 w-3.5 text-pass" />
                ) : (
                  <Copy className="h-3.5 w-3.5" />
                )}
              </Button>
              <Button
                variant="ghost"
                size="icon"
                className="h-7 w-7"
                onClick={clearLogs}
                disabled={task.running}
                title={t("overview.taskClearLogs")}
              >
                <Trash2 className="h-3.5 w-3.5" />
              </Button>
            </div>
          </div>
          <CardContent className="p-0">
            <div className="max-h-80 overflow-y-auto bg-inset p-4 font-mono text-caption leading-relaxed text-text-dim">
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
