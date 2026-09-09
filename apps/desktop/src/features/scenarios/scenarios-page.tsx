// The all-scenarios inventory, reached from the rail's home button. One row
// per scenario with its runtime state, plugin count and the usual actions.

import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { ArrowRight, Globe, Layers, Play, Plus, SquarePen, Terminal, Trash2 } from "lucide-react";
import { motion } from "motion/react";
import { useRouterStore } from "@/app/router";
import { useScenarioStore } from "@/app/store/scenarios";
import { useRuntimeStore } from "@/app/store/runtime";
import { usePluginStore } from "@/app/store/plugins";
import { useBusyStore } from "@/app/store/busy";
import type { Profile } from "@/shared/api/api";
import { cn } from "@/shared/lib/utils";
import { enterTransition } from "@/shared/lib/motion";
import { inferSurface } from "@/shared/lib/scenario";
import { Card } from "@/shared/ui/card";
import { Button } from "@/shared/ui/button";
import { Badge } from "@/shared/ui/badge";
import { Skeleton } from "@/shared/ui/skeleton";
import { EmptyState } from "@/shared/ui/empty-state";
import { PageBody, PageHeader } from "@/shared/ui/page";
import { SurfaceBadge } from "./surface-badge";
import { NewScenarioDialog } from "./new-scenario-dialog";
import { ScenarioRenameDialog } from "./scenario-rename-dialog";
import { ScenarioDeleteDialog } from "./scenario-delete-dialog";
import { WebRuntimeControls } from "@/features/runtime/runtime-controls";

export function ScenariosPage() {
  const { t } = useTranslation();
  const openScenario = useRouterStore((s) => s.openScenario);
  const profiles = useScenarioStore((s) => s.profiles);
  const profilesLoaded = useScenarioStore((s) => s.profilesLoaded);
  const loadProfiles = useScenarioStore((s) => s.loadProfiles);
  const instances = useRuntimeStore((s) => s.instances);
  const instancesLoaded = useRuntimeStore((s) => s.instancesLoaded);
  const loadInstances = useRuntimeStore((s) => s.loadInstances);
  const plugins = usePluginStore((s) => s.plugins);
  const pluginsLoaded = usePluginStore((s) => s.pluginsLoaded);
  const loadPlugins = usePluginStore((s) => s.loadPlugins);
  const busy = useBusyStore((s) => s.busyAction) !== null;

  const [newDialog, setNewDialog] = useState(false);
  const [renaming, setRenaming] = useState<Profile | null>(null);
  const [removing, setRemoving] = useState<Profile | null>(null);

  useEffect(() => {
    void loadProfiles();
    void loadInstances();
    void loadPlugins();
  }, [loadProfiles, loadInstances, loadPlugins]);

  const loaded = profilesLoaded && instancesLoaded && pluginsLoaded;

  const runningCount = useMemo(
    () => instances.filter((i) => i.status === "running").length,
    [instances],
  );

  const webCount = useMemo(
    () =>
      profiles.filter((p) => {
        const inst = instances.find((i) => i.profile === p.id);
        return inferSurface(p, inst?.surface ?? "undetermined") === "web";
      }).length,
    [profiles, instances],
  );

  const taskCount = profiles.length - webCount;

  const pluginCount = (profile: string) =>
    plugins.filter((plugin) => plugin.profile === profile).length;

  return (
    <PageBody className="space-y-5">
      <PageHeader
        title={t("settings.scenarios")}
        actions={
          <Button variant="secondary" size="sm" onClick={() => setNewDialog(true)}>
            <Plus className="h-4 w-4" />
            {t("settings.addProfile")}
          </Button>
        }
      />

      {/* Summary Metrics Bar */}
      {loaded && profiles.length > 0 && (
        <motion.div
          initial={{ opacity: 0, y: 4 }}
          animate={{ opacity: 1, y: 0 }}
          transition={enterTransition}
          className="grid grid-cols-2 gap-3 sm:grid-cols-4"
        >
          <Card className="flex items-center gap-3 p-3.5">
            <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-panel-3 text-text-dim">
              <Layers className="h-4 w-4" />
            </div>
            <div className="min-w-0">
              <div className="text-small text-text-faint">{t("settings.scenarios")}</div>
              <div className="text-heading font-semibold text-text tabular-nums">
                {profiles.length}
              </div>
            </div>
          </Card>

          <Card className="flex items-center gap-3 p-3.5">
            <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-pass/10 text-pass">
              <Play className="h-4 w-4" />
            </div>
            <div className="min-w-0">
              <div className="text-small text-text-faint">{t("settings.runningBadge")}</div>
              <div className="flex items-center gap-2 text-heading font-semibold text-text tabular-nums">
                <span>{runningCount}</span>
                {runningCount > 0 && (
                  <span className="relative flex h-2 w-2">
                    <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-pass opacity-75" />
                    <span className="relative inline-flex h-2 w-2 rounded-full bg-pass" />
                  </span>
                )}
              </div>
            </div>
          </Card>

          <Card className="flex items-center gap-3 p-3.5">
            <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-accent-soft text-accent">
              <Globe className="h-4 w-4" />
            </div>
            <div className="min-w-0">
              <div className="text-small text-text-faint">{t("overview.surfaceWeb")}</div>
              <div className="text-heading font-semibold text-text tabular-nums">{webCount}</div>
            </div>
          </Card>

          <Card className="flex items-center gap-3 p-3.5">
            <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-panel-3 text-text-dim">
              <Terminal className="h-4 w-4" />
            </div>
            <div className="min-w-0">
              <div className="text-small text-text-faint">{t("overview.surfaceTask")}</div>
              <div className="text-heading font-semibold text-text tabular-nums">{taskCount}</div>
            </div>
          </Card>
        </motion.div>
      )}

      {!loaded ? (
        <Skeleton className="h-40 w-full" />
      ) : profiles.length === 0 ? (
        <EmptyState
          icon={<Globe className="h-8 w-8 text-text-faint" />}
          action={
            <Button variant="secondary" size="sm" onClick={() => setNewDialog(true)}>
              <Plus className="h-4 w-4" />
              {t("settings.addProfile")}
            </Button>
          }
        >
          {t("overview.noScenarios")}
        </EmptyState>
      ) : (
        <motion.div
          initial={{ opacity: 0, y: 4 }}
          animate={{ opacity: 1, y: 0 }}
          transition={enterTransition}
        >
          <Card className="overflow-hidden">
            <div className="divide-y divide-border">
              {profiles.map((profile) => {
                const instance = instances.find((item) => item.profile === profile.id);
                const surface = inferSurface(profile, instance?.surface ?? "undetermined");
                const running = instance?.status === "running";
                const isDefault = profile.id === "web";
                const initial = (profile.name || profile.id).slice(0, 1).toUpperCase();

                return (
                  <div
                    key={profile.id}
                    className="flex flex-col gap-3 p-4 transition-colors hover:bg-hover/30 md:flex-row md:items-center"
                  >
                    {/* Avatar Icon */}
                    <div
                      className={cn(
                        "flex h-10 w-10 shrink-0 items-center justify-center rounded-lg font-bold text-caption transition-colors",
                        running
                          ? "border border-pass/30 bg-pass/10 text-pass"
                          : "border border-border bg-panel-3 text-text-dim",
                      )}
                    >
                      {initial}
                    </div>

                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-2">
                        <span
                          className={cn(
                            "h-2 w-2 shrink-0 rounded-full",
                            running ? "bg-pass shadow-[0_0_6px_var(--pass)]" : "bg-neutral",
                          )}
                        />
                        <button
                          type="button"
                          onClick={() => openScenario(profile.id)}
                          className="text-body font-semibold text-text hover:text-accent transition-colors text-left"
                        >
                          {profile.name}
                        </button>
                        <SurfaceBadge surface={surface} />
                        {running && <Badge variant="pass">{t("settings.runningBadge")}</Badge>}
                      </div>
                      <div className="mt-1 flex flex-wrap items-center gap-x-3 text-small text-text-faint">
                        {running && instance?.url && (
                          <span className="font-mono text-micro text-accent">{instance.url}</span>
                        )}
                        <span>
                          {t("settings.scenarioPluginCount", {
                            count: pluginCount(profile.id),
                          })}
                        </span>
                      </div>
                    </div>

                    <div className="flex shrink-0 flex-wrap items-center gap-2">
                      {surface === "web" && (
                        <WebRuntimeControls
                          profileId={profile.id}
                          running={running}
                          pid={instance?.pid}
                          size="sm"
                        />
                      )}
                      <Button
                        variant={running ? "primary" : "secondary"}
                        size="sm"
                        onClick={() => openScenario(profile.id)}
                      >
                        <span>{t("nav.overview")}</span>
                        <ArrowRight className="h-3.5 w-3.5" />
                      </Button>
                      <Button
                        variant="ghost"
                        size="icon"
                        onClick={() => setRenaming(profile)}
                        disabled={isDefault || busy}
                        title={isDefault ? t("settings.protectedProfile") : t("settings.rename")}
                        aria-label={t("settings.rename")}
                      >
                        <SquarePen className="h-4 w-4" />
                      </Button>
                      <Button
                        variant="ghost"
                        size="icon"
                        onClick={() => setRemoving(profile)}
                        disabled={isDefault || busy}
                        title={isDefault ? t("settings.protectedProfile") : t("settings.remove")}
                        aria-label={t("settings.remove")}
                      >
                        <Trash2 className="h-4 w-4" />
                      </Button>
                    </div>
                  </div>
                );
              })}
            </div>
          </Card>
        </motion.div>
      )}

      <NewScenarioDialog open={newDialog} onOpenChange={setNewDialog} onCreated={openScenario} />

      <ScenarioRenameDialog
        profile={renaming}
        open={renaming !== null}
        onOpenChange={(open) => !open && setRenaming(null)}
      />

      <ScenarioDeleteDialog
        profile={removing}
        open={removing !== null}
        onOpenChange={(open) => !open && setRemoving(null)}
      />
    </PageBody>
  );
}
