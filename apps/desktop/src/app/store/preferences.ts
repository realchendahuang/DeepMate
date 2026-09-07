// Persisted preferences (language / theme / tray / updates) plus the update
// check state. `loadPrefs` also applies the persisted language to i18next.

import { create } from "zustand";
import i18n from "@/i18n";
import type { UpdateInfo } from "@/shared/api/api";
import { api } from "@/shared/api/api";
import { runBusy } from "./busy";

interface PreferencesState {
  language: string;
  theme: string;
  checkUpdates: boolean;
  notifyUpdates: boolean;
  closeToTray: boolean;
  autostart: boolean;
  updateInfo: UpdateInfo | null;
  updateChecked: boolean;
  setLanguage: (language: string) => Promise<void>;
  setTheme: (theme: string) => Promise<void>;
  setCloseToTray: (enabled: boolean) => Promise<void>;
  setCheckUpdates: (enabled: boolean) => Promise<void>;
  setNotifyUpdates: (enabled: boolean) => Promise<void>;
  setAutostart: (enabled: boolean) => Promise<void>;
  checkUpdate: () => Promise<void>;
  // Download the desktop bundle for this platform, verify its checksum and
  // hand it to the OS installer. A stale banner (already up to date) clears.
  installUpdate: () => Promise<void>;
  // Hide the update banner until the next check finds a newer release.
  dismissUpdate: () => void;
  openRelease: (url: string) => Promise<void>;
  openExternal: (url: string) => Promise<void>;
  configExport: () => Promise<void>;
  configImport: () => Promise<void>;
  loadPrefs: () => Promise<void>;
}

export const usePreferencesStore = create<PreferencesState>((set, get) => ({
  language: "zh",
  theme: "system",
  checkUpdates: true,
  notifyUpdates: true,
  closeToTray: true,
  autostart: false,
  updateInfo: null,
  updateChecked: false,
  setLanguage: async (language: string) => {
    await runBusy("prefs", () => api.setLanguage(language));
    set({ language });
    i18n.changeLanguage(language);
  },
  setTheme: async (theme: string) => {
    await runBusy("prefs", () => api.setTheme(theme));
    set({ theme });
  },
  setCloseToTray: async (enabled: boolean) => {
    await runBusy("prefs", () => api.setCloseToTray(enabled));
    set({ closeToTray: enabled });
  },
  setCheckUpdates: async (enabled: boolean) => {
    await runBusy("prefs", () => api.setCheckUpdates(enabled));
    set({ checkUpdates: enabled });
  },
  setNotifyUpdates: async (enabled: boolean) => {
    await runBusy("prefs", () => api.setNotifyUpdates(enabled));
    set({ notifyUpdates: enabled });
  },
  setAutostart: async (enabled: boolean) => {
    await runBusy("prefs", () => api.autostartSet(enabled));
    set({ autostart: enabled });
  },
  configExport: async () => {
    await runBusy("config", () => api.configExport());
  },
  configImport: async () => {
    const imported = await runBusy("config", () => api.configImport());
    if (imported !== null) {
      // Re-read the persisted preferences and re-apply them to the UI.
      await get().loadPrefs();
    }
  },
  checkUpdate: async () => {
    const updateInfo = await runBusy("refresh", () => api.checkUpdate());
    set({ updateInfo, updateChecked: true });
  },
  installUpdate: async () => {
    const outcome = await runBusy("update-install", () => api.updateInstall());
    if (outcome.status === "up_to_date") {
      set({ updateInfo: null, updateChecked: true });
    }
  },
  dismissUpdate: () => {
    set({ updateInfo: null });
  },
  openRelease: async (url: string) => {
    await runBusy("refresh", () => api.openUrl(url));
  },
  // Open an external URL (e.g. a plugin's repository) in the system browser.
  openExternal: async (url: string) => {
    await runBusy("refresh", () => api.openUrl(url));
  },
  loadPrefs: async () => {
    const prefs = await runBusy("load", () => api.getConfig());
    let autostart = false;
    try {
      autostart = await api.autostartGet();
    } catch {
      // Auto-start state is cosmetic on startup; the toggle re-syncs it.
    }
    set({
      language: prefs.language,
      theme: prefs.theme,
      checkUpdates: prefs.check_updates,
      notifyUpdates: prefs.notify_updates,
      closeToTray: prefs.close_to_tray,
      autostart,
    });
    i18n.changeLanguage(prefs.language);
  },
}));
