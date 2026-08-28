// Global UI state, mirroring the old Slint bridge's UiEvent surface. A single
// zustand store holds the overview, the inventory lists, and the busy/error
// flags so every page reads from one source of truth.

import { create } from "zustand";
import { api } from "./api";
import type {
  DoctorReport,
  MarketEntry,
  MarketSourceInfo,
  Model,
  Overview,
  Plugin,
  Profile,
  Provider,
  UpdateInfo,
} from "./types";

interface AppState {
  // Overview.
  overview: Overview | null;
  // Inventory.
  profiles: Profile[];
  providers: Provider[];
  models: Model[];
  plugins: Plugin[];
  marketSources: MarketSourceInfo[];
  marketEntries: MarketEntry[];
  doctor: DoctorReport | null;
  snapshots: string[];
  // UI.
  busy: boolean;
  error: string | null;
  language: string;
  theme: string;
  checkUpdates: boolean;
  notifyUpdates: boolean;
  closeToTray: boolean;
  autostart: boolean;
  updateInfo: UpdateInfo | null;
  updateChecked: boolean;

  // Actions.
  refreshAll: () => Promise<void>;
  runtimeStart: () => Promise<void>;
  runtimeStop: () => Promise<void>;
  runtimeRestart: () => Promise<void>;
  openHarness: () => Promise<void>;
  runDoctor: () => Promise<void>;
  loadProfiles: () => Promise<void>;
  loadProviders: () => Promise<void>;
  loadModels: () => Promise<void>;
  upsertProvider: (provider: Provider) => Promise<void>;
  removeProvider: (id: string) => Promise<void>;
  upsertModel: (provider: string, model: Model) => Promise<void>;
  removeModel: (provider: string, id: string) => Promise<void>;
  createProfile: (name: string) => Promise<void>;
  removeProfile: (name: string) => Promise<void>;
  loadPlugins: () => Promise<void>;
  loadMarketSources: () => Promise<void>;
  searchMarket: (query: string) => Promise<void>;
  installPlugin: (profile: string, spec: string) => Promise<void>;
  // Install from the market with a compatibility preflight: a definite
  // "incompatible" verdict refuses the install; unknown or a failed check
  // lets it proceed (mirroring the CLI's --force-free default).
  marketInstall: (profile: string, spec: string) => Promise<void>;
  removePlugin: (profile: string, id: string) => Promise<void>;
  updatePlugin: (profile: string, id: string) => Promise<void>;
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
  openRelease: (url: string) => Promise<void>;
  configExport: () => Promise<void>;
  configImport: () => Promise<void>;
  loadPrefs: () => Promise<void>;
  snapshotExport: (name: string) => Promise<void>;
  snapshotImport: (name: string) => Promise<void>;
  loadSnapshots: () => Promise<void>;
}

// Wrap a command with busy/error handling.
async function run<T>(
  set: (partial: Partial<AppState>) => void,
  fn: () => Promise<T>,
): Promise<T> {
  set({ busy: true, error: null });
  try {
    const result = await fn();
    set({ busy: false });
    return result;
  } catch (e) {
    set({ busy: false, error: String(e) });
    throw e;
  }
}

export const useStore = create<AppState>((set) => ({
  overview: null,
  profiles: [],
  providers: [],
  models: [],
  plugins: [],
  marketSources: [],
  marketEntries: [],
  doctor: null,
  snapshots: [],
  busy: false,
  error: null,
  language: "en",
  theme: "system",
  checkUpdates: true,
  notifyUpdates: true,
  closeToTray: true,
  autostart: false,
  updateInfo: null,
  updateChecked: false,

  refreshAll: async () => {
    const overview = await run(set, () => api.refreshAll());
    set({ overview });
  },
  runtimeStart: async () => {
    await run(set, () => api.runtimeStart());
    await useStore.getState().refreshAll();
  },
  runtimeStop: async () => {
    await run(set, () => api.runtimeStop());
    await useStore.getState().refreshAll();
  },
  runtimeRestart: async () => {
    await run(set, () => api.runtimeRestart());
    await useStore.getState().refreshAll();
  },
  openHarness: async () => {
    await run(set, () => api.openHarness());
  },
  runDoctor: async () => {
    const doctor = await run(set, () => api.runDoctor());
    set({ doctor });
  },
  loadProfiles: async () => {
    const profiles = await run(set, () => api.listProfiles());
    set({ profiles });
  },
  loadProviders: async () => {
    const providers = await run(set, () => api.listProviders());
    set({ providers });
  },
  loadModels: async () => {
    const models = await run(set, () => api.listModels());
    set({ models });
  },
  upsertProvider: async (provider: Provider) => {
    await run(set, () => api.upsertProvider(provider));
    await useStore.getState().loadProviders();
  },
  removeProvider: async (id: string) => {
    await run(set, () => api.removeProvider(id));
    await useStore.getState().loadProviders();
    await useStore.getState().loadModels();
  },
  upsertModel: async (provider: string, model: Model) => {
    await run(set, () => api.upsertModel(provider, model));
    await useStore.getState().loadModels();
  },
  removeModel: async (provider: string, id: string) => {
    await run(set, () => api.removeModel(provider, id));
    await useStore.getState().loadModels();
  },
  createProfile: async (name: string) => {
    await run(set, () => api.createProfile(name));
    await useStore.getState().loadProfiles();
  },
  removeProfile: async (name: string) => {
    await run(set, () => api.removeProfile(name));
    await useStore.getState().loadProfiles();
  },
  loadPlugins: async () => {
    const plugins = await run(set, () => api.listPlugins());
    set({ plugins });
  },
  loadMarketSources: async () => {
    const marketSources = await run(set, () => api.listMarketSources());
    set({ marketSources });
  },
  searchMarket: async (query: string) => {
    const marketEntries = await run(set, () => api.marketSearch(query));
    set({ marketEntries });
  },
  installPlugin: async (profile: string, spec: string) => {
    await run(set, () => api.pluginInstall(profile, spec));
    await useStore.getState().loadPlugins();
  },
  marketInstall: async (profile: string, spec: string) => {
    await run(set, async () => {
      const report = await api.pluginCheck(spec);
      if (report.status === "incompatible") {
        throw new Error(report.message);
      }
      return api.pluginInstall(profile, spec);
    });
    await useStore.getState().loadPlugins();
  },
  removePlugin: async (profile: string, id: string) => {
    await run(set, () => api.pluginRemove(profile, id));
    await useStore.getState().loadPlugins();
  },
  updatePlugin: async (profile: string, id: string) => {
    await run(set, () => api.pluginUpdate(profile, id));
    await useStore.getState().loadPlugins();
  },
  setLanguage: async (language: string) => {
    await run(set, () => api.setLanguage(language));
    set({ language });
    (await import("./i18n")).default.changeLanguage(language);
  },
  setTheme: async (theme: string) => {
    await run(set, () => api.setTheme(theme));
    set({ theme });
  },
  setCloseToTray: async (enabled: boolean) => {
    await run(set, () => api.setCloseToTray(enabled));
    set({ closeToTray: enabled });
  },
  setCheckUpdates: async (enabled: boolean) => {
    await run(set, () => api.setCheckUpdates(enabled));
    set({ checkUpdates: enabled });
  },
  setNotifyUpdates: async (enabled: boolean) => {
    await run(set, () => api.setNotifyUpdates(enabled));
    set({ notifyUpdates: enabled });
  },
  setAutostart: async (enabled: boolean) => {
    await run(set, () => api.autostartSet(enabled));
    set({ autostart: enabled });
  },
  configExport: async () => {
    await run(set, () => api.configExport());
  },
  configImport: async () => {
    const imported = await run(set, () => api.configImport());
    if (imported !== null) {
      // Re-read the persisted preferences and re-apply them to the UI.
      await useStore.getState().loadPrefs();
    }
  },
  checkUpdate: async () => {
    const updateInfo = await run(set, () => api.checkUpdate());
    set({ updateInfo, updateChecked: true });
  },
  installUpdate: async () => {
    const outcome = await run(set, () => api.updateInstall());
    if (outcome.status === "up_to_date") {
      set({ updateInfo: null, updateChecked: true });
    }
  },
  openRelease: async (url: string) => {
    await run(set, () => api.openUrl(url));
  },
  loadPrefs: async () => {
    const prefs = await run(set, () => api.getConfig());
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
    (await import("./i18n")).default.changeLanguage(prefs.language);
  },
  snapshotExport: async (name: string) => {
    await run(set, () => api.snapshotExport(name));
    await useStore.getState().loadSnapshots();
  },
  snapshotImport: async (name: string) => {
    await run(set, () => api.snapshotImport(name));
    await useStore.getState().refreshAll();
  },
  loadSnapshots: async () => {
    const snapshots = await run(set, () => api.snapshotList());
    set({ snapshots });
  },
}));
