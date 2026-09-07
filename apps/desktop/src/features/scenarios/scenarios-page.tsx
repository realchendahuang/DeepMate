// The all-scenarios inventory, reached from the rail's home button. One row
// per scenario with its runtime state, plugin count and the usual actions.

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Globe, Plus, SquarePen, Trash2 } from "lucide-react";
import { useRouterStore } from "@/app/router";
import { useScenarioStore } from "@/app/store/scenarios";
import { useRuntimeStore } from "@/app/store/runtime";
import { usePluginStore } from "@/app/store/plugins";
import { useBusyStore } from "@/app/store/busy";
import type { Profile } from "@/shared/api/api";
import { cn } from "@/shared/lib/utils";
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
        <Card className="overflow-hidden">
          <div className="divide-y divide-border">
            {profiles.map((profile) => {
              const instance = instances.find((item) => item.profile === profile.id);
              const surface = inferSurface(profile, instance?.surface ?? "undetermined");
              const running = instance?.status === "running";
              const isDefault = profile.id === "web";
              return (
                <div
                  key={profile.id}
                  className="flex flex-col gap-3 p-4 md:flex-row md:items-center"
                >
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <span
                        className={cn(
                          "h-2 w-2 shrink-0 rounded-full",
                          running ? "bg-pass" : "bg-neutral",
                        )}
                      />
                      <span className="text-body font-semibold text-text">{profile.name}</span>
                      <SurfaceBadge surface={surface} />
                      {running && <Badge variant="pass">{t("settings.runningBadge")}</Badge>}
                    </div>
                    <div className="mt-0.5 flex flex-wrap gap-x-3 text-small text-text-faint">
                      {running && instance?.url && <span>{instance.url}</span>}
                      <span>
                        {t("settings.scenarioPluginCount", {
                          count: pluginCount(profile.id),
                        })}
                      </span>
                    </div>
                  </div>
                  <div className="flex shrink-0 flex-wrap items-center gap-2">
                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={() => openScenario(profile.id)}
                    >
                      {t("nav.overview")}
                    </Button>
                    {surface === "web" && (
                      <WebRuntimeControls
                        profileId={profile.id}
                        running={running}
                        pid={instance?.pid}
                        size="sm"
                      />
                    )}
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
