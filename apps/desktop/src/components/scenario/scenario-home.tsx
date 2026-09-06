// The scenario pages. The app is tenant-first: the outermost rail switches
// scenarios, the main sidebar navigates inside the selected scenario (run /
// providers & models / plugins), and each of those sections renders here.
// System-wide settings render in the Settings page instead.

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  AlertTriangle,
  CheckCircle2,
  Download,
  ExternalLink,
  Loader2,
  Pencil,
  Play,
  RotateCw,
  Square,
  SquarePen,
  TerminalSquare,
  Trash2,
  Globe,
} from "lucide-react";
import { useStore } from "../../store";
import { api } from "../../api";
import type { Model, Profile, Surface } from "../../api";
import type { View } from "../layout/nav";
import { cn } from "../../lib/utils";
import { Card, CardContent } from "../ui/card";
import { Button } from "../ui/button";
import { Badge } from "../ui/badge";
import { Input } from "../ui/input";
import { Label } from "../ui/label";
import { Skeleton } from "../ui/skeleton";
import { EmptyState } from "../ui/empty-state";
import { ConfirmDialog } from "../ui/confirm-dialog";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../ui/dialog";
import { PageBody } from "../ui/page";
import { SurfaceBadge } from "./surface-badge";
import { ModelsPanel, NewProviderDialog, ModelDialog } from "./models-panel";
import { ScenarioPlugins } from "./scenario-detail";

export function ScenarioHome({ section }: { section: View }) {
  const selectedScenario = useStore((s) => s.selectedScenario);
  const setSelectedScenario = useStore((s) => s.setSelectedScenario);
  const profile = useStore((s) => s.profiles.find((p) => p.id === selectedScenario) ?? null);

  if (!profile) {
    return <ScenarioCreator onCreate={(id) => setSelectedScenario(id)} />;
  }
  if (section === "providers") {
    return <ProvidersView key={profile.id} profile={profile} />;
  }
  if (section === "plugins") {
    return (
      <PageBody className="space-y-5">
        <ScenarioPlugins profile={profile} />
      </PageBody>
    );
  }
  return <RunView key={profile.id} profile={profile} />;
}

// ---- Creator: shown when the selection is empty / no scenario exists ----

function ScenarioCreator({ onCreate }: { onCreate: (id: string) => void }) {
  const { t } = useTranslation();
  const createScenario = useStore((s) => s.createScenario);
  const [name, setName] = useState("");
  const [surface, setSurface] = useState<Surface>("web");
  const busy = useStore((s) => s.busyAction) !== null;

  const create = () => {
    const trimmed = name.trim();
    if (!trimmed || busy) return;
    createScenario(trimmed, surface);
    onCreate(trimmed);
  };

  return (
    <PageBody className="space-y-5">
      <Card>
        <CardContent className="p-4 md:p-5">
          <div className="text-display font-bold text-text">{t("settings.newScenarioTitle")}</div>
          <p className="mt-1 text-small text-text-dim">{t("overview.noScenarios")}</p>
          <div className="mt-4 max-w-xl space-y-3">
            <div className="space-y-1">
              <Label>{t("settings.profileName")}</Label>
              <Input
                value={name}
                onChange={(event) => setName(event.target.value)}
                placeholder={t("settings.profileName")}
                autoFocus
                onKeyDown={(event) => {
                  if (event.key === "Enter") create();
                }}
              />
            </div>
            <div className="space-y-1">
              <Label>{t("settings.surfaceLabel")}</Label>
              <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
                <button
                  type="button"
                  onClick={() => setSurface("web")}
                  className={cn(
                    "rounded-md border p-3 text-left transition-colors",
                    surface === "web"
                      ? "border-accent/40 bg-accent/10"
                      : "border-border bg-panel-2 hover:bg-hover",
                  )}
                >
                  <div className="flex items-center gap-2 text-body font-semibold text-text">
                    <Globe className="h-4 w-4 text-accent" />
                    {t("settings.surfaceWeb")}
                  </div>
                  <div className="mt-1 text-small text-text-dim">
                    {t("settings.surfaceWebHint")}
                  </div>
                </button>
                <button
                  type="button"
                  onClick={() => setSurface("task")}
                  className={cn(
                    "rounded-md border p-3 text-left transition-colors",
                    surface === "task"
                      ? "border-accent/40 bg-accent/10"
                      : "border-border bg-panel-2 hover:bg-hover",
                  )}
                >
                  <div className="flex items-center gap-2 text-body font-semibold text-text">
                    <TerminalSquare className="h-4 w-4 text-accent" />
                    {t("settings.surfaceTask")}
                  </div>
                  <div className="mt-1 text-small text-text-dim">
                    {t("settings.surfaceTaskHint")}
                  </div>
                </button>
              </div>
            </div>
            <Button variant="primary" onClick={create} disabled={busy || !name.trim()}>
              {t("settings.addProfile")}
            </Button>
          </div>
        </CardContent>
      </Card>
    </PageBody>
  );
}

// ---- Run: identity, runtime controls and (for task scenarios) the runner ----

function RunView({ profile }: { profile: Profile }) {
  const { t } = useTranslation();
  const overview = useStore((s) => s.overview);
  const instances = useStore((s) => s.instances);
  const loadInstances = useStore((s) => s.loadInstances);
  const runtimeStart = useStore((s) => s.runtimeStart);
  const runtimeStop = useStore((s) => s.runtimeStop);
  const runtimeRestart = useStore((s) => s.runtimeRestart);
  const openHarness = useStore((s) => s.openHarness);
  const loadProfiles = useStore((s) => s.loadProfiles);
  const renameProfile = useStore((s) => s.renameProfile);
  const removeProfile = useStore((s) => s.removeProfile);
  const busyAction = useStore((s) => s.busyAction);

  const instance = instances.find((item) => item.profile === profile.id);
  const surface = instance?.surface ?? "undetermined";
  const running = instance?.status === "running";
  const busy = busyAction !== null;

  const [renaming, setRenaming] = useState(false);
  const [renameName, setRenameName] = useState(profile.name);
  const [removing, setRemoving] = useState(false);

  const [taskPrompt, setTaskPrompt] = useState("");
  const [taskRunning, setTaskRunning] = useState(false);
  const [taskDone, setTaskDone] = useState(false);
  const [taskFailed, setTaskFailed] = useState(false);
  const [taskLines, setTaskLines] = useState<string[]>([]);

  useEffect(() => {
    loadInstances();
  }, [loadInstances, profile.id]);

  const doRename = () => {
    const trimmed = renameName.trim();
    if (!trimmed || trimmed === profile.name) return;
    renameProfile(profile.id, trimmed);
    setRenaming(false);
    useStore.getState().setSelectedScenario(trimmed);
  };

  const doRemove = () => {
    removeProfile(profile.id);
    setRemoving(false);
    useStore.getState().setSelectedScenario("");
    loadProfiles();
  };

  const runTask = async () => {
    const prompt = taskPrompt.trim();
    if (!prompt || taskRunning) return;
    setTaskRunning(true);
    setTaskDone(false);
    setTaskFailed(false);
    setTaskLines([]);
    try {
      await api.taskRun(profile.id, prompt, (event) => {
        if (event.phase === "line") {
          setTaskLines((lines) => [...lines, event.text]);
        } else if (event.phase === "finished") {
          setTaskDone(true);
          setTaskFailed(!event.ok);
        }
      });
    } catch {
      setTaskDone(true);
      setTaskFailed(true);
    } finally {
      setTaskRunning(false);
    }
  };

  const isDefault = profile.id === "web";
  const engineMissing = overview !== null && !overview.detection.found;

  return (
    <PageBody className="space-y-5">
      {engineMissing && (
        <Card className="border-warn/40">
          <CardContent className="flex flex-col gap-2 p-4 md:flex-row md:items-center md:justify-between">
            <div className="min-w-0">
              <div className="text-heading font-semibold text-text">
                {t("overview.notDetected")}
              </div>
              <p className="mt-0.5 text-small text-text-dim">{t("overview.notDetectedHint")}</p>
            </div>
            <code className="shrink-0 rounded border border-border bg-panel-2 px-2 py-1 font-mono text-caption text-text-dim">
              {t("doctor.installCommand")}
            </code>
          </CardContent>
        </Card>
      )}

      <Card>
        <CardContent className="p-4 md:p-5">
          <div className="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
            <div className="min-w-0">
              <div className="flex flex-wrap items-center gap-2">
                <h2 className="text-display font-bold text-text">{profile.name}</h2>
                <SurfaceBadge surface={surface} />
                {running && <Badge variant="pass">{t("settings.runningBadge")}</Badge>}
              </div>
              <div className="mt-1 text-small text-text-faint">
                {profile.description ??
                  (running
                    ? (instance?.url ?? t("overview.scenarioReady"))
                    : t("settings.scenarioStopped"))}
              </div>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              {surface === "web" &&
                (running ? (
                  <>
                    <Button
                      variant="primary"
                      onClick={() => openHarness(profile.id)}
                      disabled={busy}
                    >
                      <ExternalLink className="h-4 w-4" />
                      {t("overview.openHarness")}
                    </Button>
                    <Button
                      onClick={() => runtimeRestart(profile.id)}
                      disabled={busy || instance?.pid == null}
                    >
                      <RotateCw className="h-4 w-4" />
                      {t("overview.restart")}
                    </Button>
                    <Button
                      variant="danger"
                      onClick={() => runtimeStop(profile.id)}
                      disabled={busy || instance?.pid == null}
                    >
                      <Square className="h-4 w-4" />
                      {t("overview.stop")}
                    </Button>
                  </>
                ) : (
                  <Button variant="primary" onClick={() => runtimeStart(profile.id)} disabled={busy}>
                    <Play className="h-4 w-4" />
                    {t("overview.start")}
                  </Button>
                ))}
              <Button
                variant="ghost"
                size="icon"
                onClick={() => {
                  setRenaming(true);
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
                onClick={() => setRemoving(true)}
                disabled={isDefault}
                title={isDefault ? t("settings.protectedProfile") : t("settings.remove")}
                aria-label={t("settings.remove")}
              >
                <Trash2 className="h-4 w-4" />
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      {running && (
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

      {surface === "task" && (
        <section className="space-y-3">
          <div className="text-small font-bold text-text-dim">{t("settings.runTask")}</div>
          <Card>
            <CardContent className="flex flex-col gap-2 p-4 md:flex-row">
              <Input
                value={taskPrompt}
                onChange={(event) => setTaskPrompt(event.target.value)}
                placeholder={t("settings.taskPrompt")}
                className="flex-1"
                disabled={taskRunning}
                onKeyDown={(event) => {
                  if (event.key === "Enter") runTask();
                }}
              />
              <Button
                variant="primary"
                onClick={runTask}
                disabled={taskRunning || !taskPrompt.trim()}
              >
                {taskRunning ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <TerminalSquare className="h-4 w-4" />
                )}
                {taskRunning ? t("settings.taskRunning") : t("settings.runTask")}
              </Button>
            </CardContent>
          </Card>
          {taskLines.length > 0 && (
            <Card>
              <CardContent className="flex flex-col gap-2 p-4">
                <div className="flex items-center gap-2">
                  {taskFailed ? (
                    <AlertTriangle className="h-4 w-4 shrink-0 text-fail" />
                  ) : taskDone ? (
                    <CheckCircle2 className="h-4 w-4 shrink-0 text-pass" />
                  ) : (
                    <Loader2 className="h-4 w-4 shrink-0 animate-spin text-accent" />
                  )}
                  <span className="text-small font-medium text-text">
                    {taskFailed
                      ? t("settings.taskFailed")
                      : taskDone
                        ? t("settings.taskSuccess")
                        : t("settings.taskRunning")}
                  </span>
                </div>
                <div className="max-h-72 overflow-y-auto rounded-md border border-border bg-inset p-3 font-mono text-caption text-text-dim">
                  {taskLines.map((line, index) => (
                    <div key={index} className="whitespace-pre-wrap break-words">
                      {line}
                    </div>
                  ))}
                </div>
              </CardContent>
            </Card>
          )}
        </section>
      )}

      <Dialog open={renaming} onOpenChange={(open) => !open && setRenaming(false)}>
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
              if (event.key === "Escape") setRenaming(false);
            }}
          />
          <DialogFooter>
            <Button variant="secondary" onClick={() => setRenaming(false)}>
              {t("settings.cancel")}
            </Button>
            <Button
              variant="primary"
              disabled={!renameName.trim() || renameName.trim() === profile.name}
              onClick={doRename}
            >
              {t("settings.save")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <ConfirmDialog
        open={removing}
        onOpenChange={(open) => !open && setRemoving(false)}
        title={t("settings.deleteProfileConfirmTitle")}
        body={t("settings.deleteProfileConfirmBody", { name: profile.name })}
        onConfirm={doRemove}
      />
    </PageBody>
  );
}

// ---- Providers & models: owned by this scenario ----

function ProvidersView({ profile }: { profile: Profile }) {
  const { t } = useTranslation();
  const loadProviders = useStore((s) => s.loadProviders);
  const loadModels = useStore((s) => s.loadModels);
  const removeProvider = useStore((s) => s.removeProvider);
  const removeModel = useStore((s) => s.removeModel);
  const providers = useStore((s) => s.providers);
  const models = useStore((s) => s.models);

  const [loaded, setLoaded] = useState(false);
  const [providerId, setProviderId] = useState<string | null>(null);
  const [modelDialog, setModelDialog] = useState<Model | "new" | null>(null);
  const [newProviderDialog, setNewProviderDialog] = useState(false);

  useEffect(() => {
    setLoaded(false);
    Promise.all([loadProviders(profile.id), loadModels(profile.id)]).then(() => setLoaded(true));
  }, [loadProviders, loadModels, profile.id]);

  const selected = providers.find((provider) => provider.id === providerId) ?? null;
  useEffect(() => {
    if (providers.length === 0) return;
    if (!selected) setProviderId(providers[0].id);
  }, [providers, selected]);

  return (
    <PageBody className="space-y-5">
      {!loaded ? (
        <Skeleton className="h-96 w-full" />
      ) : providers.length === 0 ? (
        <EmptyState
          icon={<Pencil className="h-8 w-8 text-text-faint" />}
          action={
            <Button variant="secondary" size="sm" onClick={() => setNewProviderDialog(true)}>
              <Download className="h-4 w-4" />
              {t("settings.addProvider")}
            </Button>
          }
        >
          {t("settings.noProviders")}
        </EmptyState>
      ) : (
        <ModelsPanel
          profile={profile.id}
          providers={providers}
          models={models}
          selectedId={selected?.id ?? null}
          onSelect={setProviderId}
          onNewProvider={() => setNewProviderDialog(true)}
          onDeleteProvider={(provider) => removeProvider(profile.id, provider.id)}
          onNewModel={() => setModelDialog("new")}
          onEditModel={(model) => setModelDialog(model)}
          onDeleteModel={(model) => removeModel(profile.id, model.provider ?? "", model.id)}
        />
      )}

      <NewProviderDialog
        profile={profile.id}
        open={newProviderDialog}
        onClose={() => setNewProviderDialog(false)}
        onCreated={(id) => setProviderId(id)}
      />
      <ModelDialog
        key={
          modelDialog === "new"
            ? `new-${providerId ?? "none"}`
            : (modelDialog?.id ?? "closed")
        }
        profile={profile.id}
        open={modelDialog !== null}
        value={modelDialog === "new" ? null : modelDialog}
        defaultProvider={providerId ?? ""}
        onClose={() => setModelDialog(null)}
      />
    </PageBody>
  );
}
