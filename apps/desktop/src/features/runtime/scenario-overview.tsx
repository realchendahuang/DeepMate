// A scenario's home: identity card with runtime controls, a restart hint
// while running, and (for task-surface scenarios) the one-shot task runner.
// Providers and plugins stay in their own sidebar sections — this page does
// not preview them.

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { RotateCw, SquarePen, Trash2 } from "lucide-react";
import { useRuntimeStore } from "@/app/store/runtime";
import { useBusyStore } from "@/app/store/busy";
import type { Profile } from "@/shared/api/api";
import { cn } from "@/shared/lib/utils";
import { inferSurface, scenarioInitial, scenarioLayers } from "@/shared/lib/scenario";
import { Card, CardContent } from "@/shared/ui/card";
import { Button } from "@/shared/ui/button";
import { Badge } from "@/shared/ui/badge";
import { PageBody } from "@/shared/ui/page";
import { SurfaceBadge } from "@/features/scenarios/surface-badge";
import { ScenarioRenameDialog } from "@/features/scenarios/scenario-rename-dialog";
import { ScenarioDeleteDialog } from "@/features/scenarios/scenario-delete-dialog";
import { WebRuntimeControls } from "./runtime-controls";
import { TaskRunner } from "./task-runner";
import { EngineMissingBanner } from "./engine-banner";

export function ScenarioOverview({ profile }: { profile: Profile }) {
  const { t } = useTranslation();
  const instances = useRuntimeStore((s) => s.instances);
  const loadInstances = useRuntimeStore((s) => s.loadInstances);
  const runtimeRestart = useRuntimeStore((s) => s.runtimeRestart);
  const busy = useBusyStore((s) => s.busyAction) !== null;

  const [renaming, setRenaming] = useState(false);
  const [removing, setRemoving] = useState(false);

  useEffect(() => {
    void loadInstances();
  }, [loadInstances, profile.id]);

  const instance = instances.find((item) => item.profile === profile.id);
  const surface = inferSurface(profile, instance?.surface ?? "undetermined");
  const running = instance?.status === "running";
  const isDefault = profile.id === "web";
  const { blurb } = scenarioLayers(profile.description);

  const subtitle = blurb
    ? blurb
    : running
      ? (instance?.url ?? t("overview.scenarioReady"))
      : surface === "task"
        ? t("overview.taskHint")
        : t("scene.stoppedHint");

  return (
    <PageBody className="space-y-5">
      <EngineMissingBanner />

      <Card>
        <CardContent className="p-5">
          <div className="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
            <div className="flex min-w-0 items-start gap-3">
              <div
                className={cn(
                  "flex h-12 w-12 shrink-0 items-center justify-center rounded-2xl text-title font-bold",
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
                  {surface !== "undetermined" && <SurfaceBadge surface={surface} />}
                  <Badge variant={running ? "pass" : "neutral"}>
                    {running ? t("settings.runningBadge") : t("settings.scenarioStopped")}
                  </Badge>
                </div>
                <p className="mt-1 truncate text-small text-text-dim">{subtitle}</p>
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
                <WebRuntimeControls
                  profileId={profile.id}
                  running={running}
                  pid={instance?.pid}
                />
              )}
              <Button
                variant="ghost"
                size="icon"
                onClick={() => setRenaming(true)}
                disabled={isDefault || busy}
                title={isDefault ? t("settings.protectedProfile") : t("settings.rename")}
                aria-label={t("settings.rename")}
              >
                <SquarePen className="h-4 w-4" />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                onClick={() => setRemoving(true)}
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

      {running && surface === "web" && (
        <Card className="border-accent/40">
          <CardContent className="flex flex-col gap-2 p-3 md:flex-row md:items-center md:justify-between">
            <span className="text-small text-text-dim">{t("settings.restartToApply")}</span>
            <Button size="sm" onClick={() => runtimeRestart(profile.id)} disabled={busy}>
              <RotateCw className="h-4 w-4" />
              {t("overview.restart")}
            </Button>
          </CardContent>
        </Card>
      )}

      {surface === "task" && <TaskRunner profileId={profile.id} />}

      <ScenarioRenameDialog profile={profile} open={renaming} onOpenChange={setRenaming} />

      <ScenarioDeleteDialog profile={profile} open={removing} onOpenChange={setRemoving} />
    </PageBody>
  );
}
