// The scenario management page: every scenario in one place. Rows show the
// surface, runtime state, address and plugin count with per-scenario
// controls (start/stop/restart/open, rename, delete), and a "new scenario"
// dialog sits in the page header. Reached from the grid button at the top
// of the scenario rail.

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  ExternalLink,
  Globe,
  Play,
  Plus,
  RotateCw,
  Square,
  SquarePen,
  TerminalSquare,
  Trash2,
} from "lucide-react";
import { useStore } from "../store";
import type { Profile, Surface } from "../api";
import { cn } from "../lib/utils";
import { Card } from "../components/ui/card";
import { Button } from "../components/ui/button";
import { Badge } from "../components/ui/badge";
import { Input } from "../components/ui/input";
import { Label } from "../components/ui/label";
import { Skeleton } from "../components/ui/skeleton";
import { EmptyState } from "../components/ui/empty-state";
import { ConfirmDialog } from "../components/ui/confirm-dialog";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../components/ui/dialog";
import { PageBody, PageHeader } from "../components/ui/page";
import { SurfaceBadge } from "../components/scenario/surface-badge";

export function ScenariosAdmin({ onOpenScenario }: { onOpenScenario: (profile: string) => void }) {
  const { t } = useTranslation();
  const profiles = useStore((s) => s.profiles);
  const instances = useStore((s) => s.instances);
  const plugins = useStore((s) => s.plugins);
  const busyAction = useStore((s) => s.busyAction);
  const loadProfiles = useStore((s) => s.loadProfiles);
  const loadInstances = useStore((s) => s.loadInstances);
  const loadPlugins = useStore((s) => s.loadPlugins);
  const createScenario = useStore((s) => s.createScenario);
  const renameProfile = useStore((s) => s.renameProfile);
  const removeProfile = useStore((s) => s.removeProfile);
  const runtimeStart = useStore((s) => s.runtimeStart);
  const runtimeStop = useStore((s) => s.runtimeStop);
  const runtimeRestart = useStore((s) => s.runtimeRestart);
  const openHarness = useStore((s) => s.openHarness);

  const busy = busyAction !== null;
  const [loaded, setLoaded] = useState(false);

  const [newDialog, setNewDialog] = useState(false);
  const [newName, setNewName] = useState("");
  const [newSurface, setNewSurface] = useState<Surface>("web");
  const [renaming, setRenaming] = useState<Profile | null>(null);
  const [renameName, setRenameName] = useState("");
  const [removing, setRemoving] = useState<Profile | null>(null);

  useEffect(() => {
    setLoaded(false);
    Promise.all([loadProfiles(), loadInstances(), loadPlugins()]).then(() => setLoaded(true));
  }, [loadProfiles, loadInstances, loadPlugins]);

  const create = () => {
    const trimmed = newName.trim();
    if (!trimmed) return;
    createScenario(trimmed, newSurface);
    setNewDialog(false);
    setNewName("");
    onOpenScenario(trimmed);
  };

  const doRename = () => {
    if (!renaming) return;
    const trimmed = renameName.trim();
    if (!trimmed || trimmed === renaming.name) return;
    renameProfile(renaming.id, trimmed);
    setRenaming(null);
  };

  const doRemove = () => {
    if (!removing) return;
    removeProfile(removing.id);
    setRemoving(null);
  };

  const pluginCount = (profile: string) =>
    plugins.filter((plugin) => plugin.profile === profile).length;

  return (
    <PageBody className="space-y-5">
      <PageHeader
        title={t("settings.scenarios")}
        actions={
          <Button
            variant="secondary"
            size="sm"
            onClick={() => {
              setNewName("");
              setNewDialog(true);
            }}
          >
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
              const surface = instance?.surface ?? "undetermined";
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
                      {profile.description && <span>{profile.description}</span>}
                    </div>
                  </div>
                  <div className="flex shrink-0 flex-wrap items-center gap-2">
                    <Button variant="secondary" size="sm" onClick={() => onOpenScenario(profile.id)}>
                      {t("nav.run")}
                    </Button>
                    {surface === "web" &&
                      (running ? (
                        <>
                          <Button
                            variant="primary"
                            size="sm"
                            onClick={() => openHarness(profile.id)}
                            disabled={busy}
                          >
                            <ExternalLink className="h-4 w-4" />
                            {t("overview.openHarness")}
                          </Button>
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => runtimeRestart(profile.id)}
                            disabled={busy || instance?.pid == null}
                          >
                            <RotateCw className="h-4 w-4" />
                            {t("overview.restart")}
                          </Button>
                          <Button
                            variant="danger"
                            size="sm"
                            onClick={() => runtimeStop(profile.id)}
                            disabled={busy || instance?.pid == null}
                          >
                            <Square className="h-4 w-4" />
                            {t("overview.stop")}
                          </Button>
                        </>
                      ) : (
                        <Button
                          variant="primary"
                          size="sm"
                          onClick={() => runtimeStart(profile.id)}
                          disabled={busy}
                        >
                          <Play className="h-4 w-4" />
                          {t("overview.start")}
                        </Button>
                      ))}
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={() => {
                        setRenaming(profile);
                        setRenameName(profile.name);
                      }}
                      disabled={isDefault}
                      title={isDefault ? t("settings.protectedProfile") : t("settings.rename")}
                      aria-label={t("settings.rename")}
                    >
                      <SquarePen className="h-4 w-4" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={() => setRemoving(profile)}
                      disabled={isDefault}
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

      {/* New scenario */}
      <Dialog open={newDialog} onOpenChange={(open) => !open && setNewDialog(false)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("settings.newScenarioTitle")}</DialogTitle>
          </DialogHeader>
          <div className="space-y-2">
            <Label>{t("settings.surfaceLabel")}</Label>
            <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
              <button
                type="button"
                onClick={() => setNewSurface("web")}
                className={cn(
                  "rounded-md border p-3 text-left transition-colors",
                  newSurface === "web"
                    ? "border-accent/40 bg-accent/10"
                    : "border-border bg-panel-2 hover:bg-hover",
                )}
              >
                <div className="flex items-center gap-2 text-body font-semibold text-text">
                  <Globe className="h-4 w-4 text-accent" />
                  {t("settings.surfaceWeb")}
                </div>
                <div className="mt-1 text-small text-text-dim">{t("settings.surfaceWebHint")}</div>
              </button>
              <button
                type="button"
                onClick={() => setNewSurface("task")}
                className={cn(
                  "rounded-md border p-3 text-left transition-colors",
                  newSurface === "task"
                    ? "border-accent/40 bg-accent/10"
                    : "border-border bg-panel-2 hover:bg-hover",
                )}
              >
                <div className="flex items-center gap-2 text-body font-semibold text-text">
                  <TerminalSquare className="h-4 w-4 text-accent" />
                  {t("settings.surfaceTask")}
                </div>
                <div className="mt-1 text-small text-text-dim">{t("settings.surfaceTaskHint")}</div>
              </button>
            </div>
          </div>
          <Input
            value={newName}
            onChange={(event) => setNewName(event.target.value)}
            placeholder={t("settings.profileName")}
            autoFocus
            onKeyDown={(event) => {
              if (event.key === "Enter") create();
            }}
          />
          <DialogFooter>
            <Button variant="secondary" onClick={() => setNewDialog(false)}>
              {t("settings.cancel")}
            </Button>
            <Button variant="primary" disabled={!newName.trim()} onClick={create}>
              {t("settings.addProfile")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Rename */}
      <Dialog open={renaming !== null} onOpenChange={(open) => !open && setRenaming(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("settings.renameScenarioTitle")}</DialogTitle>
          </DialogHeader>
          <Input
            value={renameName}
            onChange={(event) => setRenameName(event.target.value)}
            placeholder={t("settings.profileName")}
            autoFocus
            onKeyDown={(event) => {
              if (event.key === "Enter") doRename();
              if (event.key === "Escape") setRenaming(null);
            }}
          />
          <DialogFooter>
            <Button variant="secondary" onClick={() => setRenaming(null)}>
              {t("settings.cancel")}
            </Button>
            <Button
              variant="primary"
              disabled={!renameName.trim() || renameName.trim() === renaming?.name}
              onClick={doRename}
            >
              {t("settings.save")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Delete */}
      <ConfirmDialog
        open={removing !== null}
        onOpenChange={(open) => !open && setRemoving(null)}
        title={t("settings.deleteProfileConfirmTitle")}
        body={t("settings.deleteProfileConfirmBody", { name: removing?.name ?? "" })}
        onConfirm={doRemove}
      />
    </PageBody>
  );
}