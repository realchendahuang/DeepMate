// Preferences: language, theme, auto-start, close-to-tray, updates and the
// own-settings backup.

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { usePreferencesStore } from "@/app/store/preferences";
import { useBusyStore } from "@/app/store/busy";
import { Card } from "@/shared/ui/card";
import { Button } from "@/shared/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/shared/ui/select";
import { Switch } from "@/shared/ui/switch";
import { ConfirmDialog } from "@/shared/ui/confirm-dialog";

export function PreferencesSection() {
  const { t } = useTranslation();

  const setLanguage = usePreferencesStore((s) => s.setLanguage);
  const setTheme = usePreferencesStore((s) => s.setTheme);
  const setCloseToTray = usePreferencesStore((s) => s.setCloseToTray);
  const setCheckUpdates = usePreferencesStore((s) => s.setCheckUpdates);
  const setNotifyUpdates = usePreferencesStore((s) => s.setNotifyUpdates);
  const setAutostart = usePreferencesStore((s) => s.setAutostart);
  const checkUpdate = usePreferencesStore((s) => s.checkUpdate);
  const installUpdate = usePreferencesStore((s) => s.installUpdate);
  const openRelease = usePreferencesStore((s) => s.openRelease);
  const configExport = usePreferencesStore((s) => s.configExport);
  const configImport = usePreferencesStore((s) => s.configImport);
  const language = usePreferencesStore((s) => s.language);
  const theme = usePreferencesStore((s) => s.theme);
  const closeToTray = usePreferencesStore((s) => s.closeToTray);
  const checkUpdates = usePreferencesStore((s) => s.checkUpdates);
  const notifyUpdates = usePreferencesStore((s) => s.notifyUpdates);
  const autostart = usePreferencesStore((s) => s.autostart);
  const updateInfo = usePreferencesStore((s) => s.updateInfo);
  const updateChecked = usePreferencesStore((s) => s.updateChecked);
  const busy = useBusyStore((s) => s.busyAction) !== null;

  const [configImportPending, setConfigImportPending] = useState(false);

  return (
    <section className="space-y-3">
      <Card>
        <div className="divide-y divide-border">
          <div className="flex flex-col gap-3 p-4 md:flex-row md:items-center md:justify-between">
            <div>
              <div className="text-heading font-semibold text-text">{t("settings.language")}</div>
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
              <div className="text-heading font-semibold text-text">{t("settings.autoStart")}</div>
            </div>
            <Switch
              checked={autostart}
              onCheckedChange={setAutostart}
              aria-label={t("settings.autoStart")}
            />
          </div>
          <div className="flex flex-col gap-3 p-4 md:flex-row md:items-center md:justify-between">
            <div>
              <div className="text-heading font-semibold text-text">{t("settings.closeToTray")}</div>
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
                <div className="text-heading font-semibold text-text">{t("settings.updates")}</div>
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
              <div className="text-heading font-semibold text-text">{t("settings.ownSettings")}</div>
            </div>
            <div className="flex flex-wrap gap-2">
              <Button variant="secondary" size="sm" onClick={configExport} disabled={busy}>
                {t("settings.exportSettings")}
              </Button>
              <Button
                variant="secondary"
                size="sm"
                onClick={() => setConfigImportPending(true)}
                disabled={busy}
              >
                {t("settings.importSettings")}
              </Button>
            </div>
          </div>
        </div>
      </Card>

      <ConfirmDialog
        open={configImportPending}
        onOpenChange={setConfigImportPending}
        title={t("settings.importConfigConfirmTitle")}
        body={t("settings.importConfigConfirmBody")}
        onConfirm={() => {
          setConfigImportPending(false);
          configImport();
        }}
      />
    </section>
  );
}
