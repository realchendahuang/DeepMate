// The settings pages behind the sidebar's footer group: snapshots (global
// migration tool), preferences (language/theme/tray/updates) and about.
// Provider/model configuration now lives per scenario in the scenario home,
// so nothing here touches inventory data.

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Trash2, Layers } from "lucide-react";
import { useStore } from "../store";
import { appVersion } from "../bindings";
import { Card, CardContent } from "../components/ui/card";
import { Button } from "../components/ui/button";
import { Input } from "../components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "../components/ui/select";
import { Skeleton } from "../components/ui/skeleton";
import { EmptyState } from "../components/ui/empty-state";
import { ConfirmDialog } from "../components/ui/confirm-dialog";
import { PageBody, PageHeader } from "../components/ui/page";
import { Switch } from "../components/ui/switch";
import { Tabs, TabsList, TabsTrigger } from "../components/ui/tabs";
import { DiagnosticsView } from "./Diagnostics";
import type { View } from "../components/layout/nav";

// The flat configuration pages behind the sidebar's footer group.
export type SettingsSection = "diagnostics" | "snapshots" | "preferences" | "about";

export function SettingsPage({
  section,
  onNavigate,
}: {
  section: SettingsSection;
  onNavigate?: (view: View) => void;
}) {
  const { t } = useTranslation();

  const setLanguage = useStore((s) => s.setLanguage);
  const setTheme = useStore((s) => s.setTheme);
  const setCloseToTray = useStore((s) => s.setCloseToTray);
  const setCheckUpdates = useStore((s) => s.setCheckUpdates);
  const setNotifyUpdates = useStore((s) => s.setNotifyUpdates);
  const setAutostart = useStore((s) => s.setAutostart);
  const checkUpdate = useStore((s) => s.checkUpdate);
  const installUpdate = useStore((s) => s.installUpdate);
  const openRelease = useStore((s) => s.openRelease);
  const configExport = useStore((s) => s.configExport);
  const configImport = useStore((s) => s.configImport);
  const language = useStore((s) => s.language);
  const theme = useStore((s) => s.theme);
  const closeToTray = useStore((s) => s.closeToTray);
  const checkUpdates = useStore((s) => s.checkUpdates);
  const notifyUpdates = useStore((s) => s.notifyUpdates);
  const autostart = useStore((s) => s.autostart);
  const updateInfo = useStore((s) => s.updateInfo);
  const updateChecked = useStore((s) => s.updateChecked);
  const busyAction = useStore((s) => s.busyAction);
  const overview = useStore((s) => s.overview);
  const refreshAll = useStore((s) => s.refreshAll);
  const snapshots = useStore((s) => s.snapshots);
  const loadSnapshots = useStore((s) => s.loadSnapshots);
  const snapshotExport = useStore((s) => s.snapshotExport);
  const snapshotImport = useStore((s) => s.snapshotImport);
  const snapshotDelete = useStore((s) => s.snapshotDelete);

  const [snapshotName, setSnapshotName] = useState("");

  // Confirm-dialog state: which snapshot/import is pending.
  const [confirm, setConfirm] = useState<{
    kind: "snapshot" | "snapshot-import" | "config-import";
    id: string;
    name: string;
  } | null>(null);

  // First-load tracking for skeletons. Each section loads only its own data.
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    const loaders: Partial<Record<SettingsSection, () => Promise<void>>> = {
      snapshots: loadSnapshots,
      // The About page shows the detected harness identity.
      about: refreshAll,
    };
    const load = loaders[section];
    setLoaded(false);
    load?.().finally(() => setLoaded(true));
  }, [section, loadSnapshots, refreshAll]);

  const busy = busyAction !== null;

  const runConfirm = () => {
    if (!confirm) return;
    const { kind, name } = confirm;
    setConfirm(null);
    if (kind === "snapshot") snapshotDelete(name);
    else if (kind === "snapshot-import") snapshotImport(name);
    else if (kind === "config-import") configImport();
  };

  const confirmTitle = t(
    `settings.${
      confirm?.kind === "snapshot-import"
        ? "importSnapshot"
        : confirm?.kind === "config-import"
          ? "importConfig"
          : "deleteSnapshot"
    }ConfirmTitle`,
  );
  const confirmBody = confirm
    ? t(
        `settings.${
          confirm.kind === "snapshot-import"
            ? "importSnapshot"
            : confirm.kind === "config-import"
              ? "importConfig"
              : "deleteSnapshot"
        }ConfirmBody`,
        { name: confirm.name },
      )
    : "";

  return (
    <PageBody className="space-y-5">
      <PageHeader
        title={t("settings.title")}
        actions={
          <Tabs value={section} onValueChange={(value) => onNavigate?.(value as View)}>
            <TabsList>
              <TabsTrigger value="diagnostics">{t("settings.diagnostics")}</TabsTrigger>
              <TabsTrigger value="snapshots">{t("settings.snapshots")}</TabsTrigger>
              <TabsTrigger value="preferences">{t("settings.preferences")}</TabsTrigger>
              <TabsTrigger value="about">{t("settings.about")}</TabsTrigger>
            </TabsList>
          </Tabs>
        }
      />

      {section === "diagnostics" && <DiagnosticsView />}

      {section === "snapshots" && (
        <section className="space-y-3">
          <Card>
            <CardContent className="flex flex-col gap-2 p-4 md:flex-row">
              <Input
                value={snapshotName}
                onChange={(event) => setSnapshotName(event.target.value)}
                placeholder={t("settings.snapshotName")}
                className="flex-1"
                onKeyDown={(event) => {
                  if (event.key === "Enter" && snapshotName.trim()) {
                    snapshotExport(snapshotName.trim());
                    setSnapshotName("");
                  }
                }}
              />
              <Button
                variant="primary"
                onClick={() => {
                  if (snapshotName.trim()) {
                    snapshotExport(snapshotName.trim());
                    setSnapshotName("");
                  }
                }}
                disabled={!snapshotName.trim()}
              >
                <Plus className="h-4 w-4" />
                {t("settings.exportSnapshot")}
              </Button>
            </CardContent>
          </Card>

          {!loaded ? (
            <Skeleton className="h-24 w-full" />
          ) : snapshots.length === 0 ? (
            <EmptyState icon={<Layers className="h-8 w-8 text-text-faint" />}>
              {t("settings.noSnapshots")}
            </EmptyState>
          ) : (
            <Card className="overflow-hidden">
              <div className="divide-y divide-border">
                {snapshots.map((name) => (
                  <div
                    key={name}
                    className="flex items-center gap-3 px-4 py-2.5 transition-colors hover:bg-hover md:px-5"
                  >
                    <div className="min-w-0 flex-1 truncate text-body font-semibold text-text">
                      {name}
                    </div>
                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={() => setConfirm({ kind: "snapshot-import", id: name, name })}
                    >
                      {t("settings.importSnapshot")}
                    </Button>
                    <Button
                      variant="danger"
                      size="sm"
                      onClick={() => setConfirm({ kind: "snapshot", id: name, name })}
                    >
                      <Trash2 className="h-4 w-4" />
                      {t("settings.deleteSnapshot")}
                    </Button>
                  </div>
                ))}
              </div>
            </Card>
          )}
        </section>
      )}

      {section === "preferences" && (
        <section className="space-y-3">
          <Card>
            <div className="divide-y divide-border">
              <div className="flex flex-col gap-3 p-4 md:flex-row md:items-center md:justify-between">
                <div>
                  <div className="text-heading font-semibold text-text">
                    {t("settings.language")}
                  </div>
                </div>
                <Select value={language} onValueChange={(value) => setLanguage(value)}>
                  <SelectTrigger className="w-[140px]">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="en">English</SelectItem>
                    <SelectItem value="zh">简体中文</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <div className="flex flex-col gap-3 p-4 md:flex-row md:items-center md:justify-between">
                <div>
                  <div className="text-heading font-semibold text-text">{t("settings.theme")}</div>
                </div>
                <Select value={theme} onValueChange={(value) => setTheme(value)}>
                  <SelectTrigger className="w-[140px]">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="system">{t("settings.themeSystem")}</SelectItem>
                    <SelectItem value="light">{t("settings.themeLight")}</SelectItem>
                    <SelectItem value="dark">{t("settings.themeDark")}</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <div className="flex flex-col gap-3 p-4 md:flex-row md:items-center md:justify-between">
                <div>
                  <div className="text-heading font-semibold text-text">
                    {t("settings.autoStart")}
                  </div>
                </div>
                <Switch
                  checked={autostart}
                  onCheckedChange={setAutostart}
                  aria-label={t("settings.autoStart")}
                />
              </div>
              <div className="flex flex-col gap-3 p-4 md:flex-row md:items-center md:justify-between">
                <div>
                  <div className="text-heading font-semibold text-text">
                    {t("settings.closeToTray")}
                  </div>
                </div>
                <Switch
                  checked={closeToTray}
                  onCheckedChange={setCloseToTray}
                  aria-label={t("settings.closeToTray")}
                />
              </div>
              <div className="space-y-3 p-4">
                <div className="flex items-center justify-between gap-3">
                  <div>
                    <div className="text-heading font-semibold text-text">
                      {t("settings.updates")}
                    </div>
                  </div>
                  <Switch
                    checked={checkUpdates}
                    onCheckedChange={setCheckUpdates}
                    aria-label={t("settings.updates")}
                  />
                </div>
                <div className="flex items-center justify-between gap-3">
                  <div>
                    <div className="text-heading font-semibold text-text">
                      {t("settings.notifications")}
                    </div>
                  </div>
                  <Switch
                    checked={notifyUpdates}
                    onCheckedChange={setNotifyUpdates}
                    aria-label={t("settings.notifications")}
                  />
                </div>
                <div className="flex flex-wrap items-center gap-2">
                  <Button variant="secondary" size="sm" onClick={checkUpdate} disabled={busy}>
                    {t("settings.checkNow")}
                  </Button>
                  {updateInfo && (
                    <>
                      <Button variant="primary" size="sm" onClick={installUpdate} disabled={busy}>
                        {t("settings.installUpdate")}
                      </Button>
                      <Button
                        variant="primary"
                        size="sm"
                        onClick={() => openRelease(updateInfo.url)}
                        title={updateInfo.url}
                      >
                        {t("settings.updateAvailable", {
                          version: updateInfo.latest_version,
                        })}
                      </Button>
                    </>
                  )}
                  {updateChecked && !updateInfo && (
                    <span className="text-small text-text-dim">{t("settings.upToDate")}</span>
                  )}
                </div>
              </div>
              <div className="flex flex-col gap-3 p-4 md:flex-row md:items-center md:justify-between">
                <div>
                  <div className="text-heading font-semibold text-text">
                    {t("settings.ownSettings")}
                  </div>
                </div>
                <div className="flex flex-wrap gap-2">
                  <Button variant="secondary" size="sm" onClick={configExport} disabled={busy}>
                    {t("settings.exportSettings")}
                  </Button>
                  <Button
                    variant="secondary"
                    size="sm"
                    onClick={() => setConfirm({ kind: "config-import", id: "", name: "" })}
                    disabled={busy}
                  >
                    {t("settings.importSettings")}
                  </Button>
                </div>
              </div>
            </div>
          </Card>
        </section>
      )}

      {section === "about" && (
        <section className="space-y-3">
          <Card>
            <CardContent className="p-4">
              <div className="text-heading font-semibold text-text">DeepMate</div>
              <div className="mt-1 text-small text-text-dim">
                {t("settings.aboutVersion", { version: appVersion })}
              </div>
            </CardContent>
          </Card>
          {overview?.detection.harness && (
            <Card>
              <CardContent className="p-4">
                <div className="text-heading font-semibold text-text">
                  {overview.detection.harness.name}
                </div>
                {overview.detection.harness.version && (
                  <div className="mt-1 text-small text-text-dim">
                    {t("settings.aboutVersion", { version: overview.detection.harness.version })}
                  </div>
                )}
              </CardContent>
            </Card>
          )}
        </section>
      )}

      <ConfirmDialog
        open={confirm !== null}
        onOpenChange={(open) => !open && setConfirm(null)}
        title={confirm ? confirmTitle : ""}
        body={confirmBody}
        onConfirm={runConfirm}
      />
    </PageBody>
  );
}