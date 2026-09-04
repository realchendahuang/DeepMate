// Global UI state, mirroring the old Slint bridge's UiEvent surface. A single
// zustand store holds the overview, the inventory lists, and the busy/error
// flags so every page reads from one source of truth.

import { create } from "zustand";
import { toast } from "sonner";
import { api } from "./api";
import { mapError } from "./lib/errors";
import type {
  DoctorReport,
  MarketEntry,
  MarketSourceInfo,
  Model,
  Overview,
  Plugin,
  PluginOpEvent,
  PluginOpKind,
  Profile,
  Provider,
  UpdateInfo,
} from "./api";

// Named operations for the busy tracker. Read-only operations (loads, search,
// doctor) only disable their own trigger; mutating operations disable
// everything that would conflict with them.
export type BusyAction =
  | "refresh"
  | "runtime"
  | "doctor"
  | "load"
  | "search"
  | "install"
  | "remove"
  | "update"
  | "save"
  | "prefs"
  | "update-install"
  | "config"
  | "snapshot";

// Mutating operations that must not run concurrently with each other.
const MUTATING: ReadonlySet<BusyAction> = new Set([
  "install",
  "remove",
  "update",
  "save",
  "prefs",
  "update-install",
  "config",
  "snapshot",
]);

// A read-only operation never blocks anything; a mutating operation blocks
// every other mutating operation (and itself).
export function isBlocked(active: BusyAction | null, candidate: BusyAction): boolean {
  if (active === null) return false;
  if (active === candidate) return true;
  return MUTATING.has(active) && MUTATING.has(candidate);
}

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
  busyAction: BusyAction | null;
  language: string;
  theme: string;
  checkUpdates: boolean;
  notifyUpdates: boolean;
  closeToTray: boolean;
  autostart: boolean;
  updateInfo: UpdateInfo | null;
  updateChecked: boolean;
  // Live plugin operation (install / remove / update): the event log the
  // progress dialog renders, and whether an operation is in flight.
  opLog: PluginOpEvent[];
  opActive: boolean;

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
  // Run a plugin operation with a live progress log. The dialog stays open
  // until the operation finishes; events append to `opLog` as they arrive.
  runPluginOp: (profile: string, kind: PluginOpKind, target: string) => Promise<void>;
  // Close the progress dialog and clear its log.
  dismissPluginOp: () => void;
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
  configExport: () => Promise<void>;
  configImport: () => Promise<void>;
  loadPrefs: () => Promise<void>;
  snapshotExport: (name: string) => Promise<void>;
  snapshotImport: (name: string) => Promise<void>;
  snapshotDelete: (name: string) => Promise<void>;
  loadSnapshots: () => Promise<void>;
}

// Wrap a command with busy/error handling. Failures surface as a toast with
// a friendly message; the raw error is rethrown for callers that need it.
async function run<T>(
  set: (partial: Partial<AppState>) => void,
  action: BusyAction,
  fn: () => Promise<T>,
): Promise<T> {
  set({ busyAction: action });
  try {
    return await fn();
  } catch (e) {
    toast.error(mapError(e));
    throw e;
  } finally {
    set({ busyAction: null });
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
  busyAction: null,
  language: "en",
  theme: "system",
  checkUpdates: true,
  notifyUpdates: true,
  closeToTray: true,
  autostart: false,
  updateInfo: null,
  updateChecked: false,
  opLog: [],
  opActive: false,

  refreshAll: async () => {
    const overview = await run(set, "refresh", () => api.refreshAll());
    set({ overview });
  },
  runtimeStart: async () => {
    await run(set, "runtime", () => api.runtimeStart());
    await useStore.getState().refreshAll();
  },
  runtimeStop: async () => {
    await run(set, "runtime", () => api.runtimeStop());
    await useStore.getState().refreshAll();
  },
  runtimeRestart: async () => {
    await run(set, "runtime", () => api.runtimeRestart());
    await useStore.getState().refreshAll();
  },
  openHarness: async () => {
    await run(set, "refresh", () => api.openHarness());
  },
  runDoctor: async () => {
    const doctor = await run(set, "doctor", () => api.runDoctor());
    set({ doctor });
  },
  loadProfiles: async () => {
    const profiles = await run(set, "load", () => api.listProfiles());
    set({ profiles });
  },
  loadProviders: async () => {
    const providers = await run(set, "load", () => api.listProviders());
    set({ providers });
  },
  loadModels: async () => {
    const models = await run(set, "load", () => api.listModels());
    set({ models });
  },
  upsertProvider: async (provider: Provider) => {
    await run(set, "save", () => api.upsertProvider(provider));
    await useStore.getState().loadProviders();
  },
  removeProvider: async (id: string) => {
    await run(set, "remove", () => api.removeProvider(id));
    await useStore.getState().loadProviders();
    await useStore.getState().loadModels();
  },
  upsertModel: async (provider: string, model: Model) => {
    await run(set, "save", () => api.upsertModel(provider, model));
    await useStore.getState().loadModels();
  },
  removeModel: async (provider: string, id: string) => {
    await run(set, "remove", () => api.removeModel(provider, id));
    await useStore.getState().loadModels();
  },
  createProfile: async (name: string) => {
    await run(set, "save", () => api.createProfile(name));
    await useStore.getState().loadProfiles();
  },
  removeProfile: async (name: string) => {
    await run(set, "remove", () => api.removeProfile(name));
    await useStore.getState().loadProfiles();
  },
  loadPlugins: async () => {
    const plugins = await run(set, "load", () => api.listPlugins());
    set({ plugins });
  },
  loadMarketSources: async () => {
    const marketSources = await run(set, "load", () => api.listMarketSources());
    set({ marketSources });
  },
  searchMarket: async (query: string) => {
    const marketEntries = await run(set, "search", () => api.marketSearch(query));
    set({ marketEntries });
  },
  installPlugin: async (profile: string, spec: string) => {
    await run(set, "install", () => api.pluginInstall(profile, spec));
    await useStore.getState().loadPlugins();
  },
  marketInstall: async (profile: string, spec: string) => {
    await run(set, "install", async () => {
      const report = await api.pluginCheck(spec);
      if (report.status === "incompatible") {
        throw new Error(report.message);
      }
      return api.pluginInstall(profile, spec);
    });
    await useStore.getState().loadPlugins();
  },
  removePlugin: async (profile: string, id: string) => {
    await run(set, "remove", () => api.pluginRemove(profile, id));
    await useStore.getState().loadPlugins();
  },
  updatePlugin: async (profile: string, id: string) => {
    await run(set, "update", () => api.pluginUpdate(profile, id));
    await useStore.getState().loadPlugins();
  },
  runPluginOp: async (profile: string, kind: PluginOpKind, target: string) => {
    set({ opLog: [], opActive: true });
    try {
      await api.pluginOpStream(profile, kind, target, (event) => {
        set((state) => ({ opLog: [...state.opLog, event] }));
      });
      await useStore.getState().loadPlugins();
    } catch (e) {
      toast.error(mapError(e));
      throw e;
    } finally {
      set({ opActive: false });
    }
  },
  dismissPluginOp: () => {
    set({ opLog: [], opActive: false });
  },
  setLanguage: async (language: string) => {
    await run(set, "prefs", () => api.setLanguage(language));
    set({ language });
    (await import("./i18n")).default.changeLanguage(language);
  },
  setTheme: async (theme: string) => {
    await run(set, "prefs", () => api.setTheme(theme));
    set({ theme });
  },
  setCloseToTray: async (enabled: boolean) => {
    await run(set, "prefs", () => api.setCloseToTray(enabled));
    set({ closeToTray: enabled });
  },
  setCheckUpdates: async (enabled: boolean) => {
    await run(set, "prefs", () => api.setCheckUpdates(enabled));
    set({ checkUpdates: enabled });
  },
  setNotifyUpdates: async (enabled: boolean) => {
    await run(set, "prefs", () => api.setNotifyUpdates(enabled));
    set({ notifyUpdates: enabled });
  },
  setAutostart: async (enabled: boolean) => {
    await run(set, "prefs", () => api.autostartSet(enabled));
    set({ autostart: enabled });
  },
  configExport: async () => {
    await run(set, "config", () => api.configExport());
  },
  configImport: async () => {
    const imported = await run(set, "config", () => api.configImport());
    if (imported !== null) {
      // Re-read the persisted preferences and re-apply them to the UI.
      await useStore.getState().loadPrefs();
    }
  },
  checkUpdate: async () => {
    const updateInfo = await run(set, "refresh", () => api.checkUpdate());
    set({ updateInfo, updateChecked: true });
  },
  installUpdate: async () => {
    const outcome = await run(set, "update-install", () => api.updateInstall());
    if (outcome.status === "up_to_date") {
      set({ updateInfo: null, updateChecked: true });
    }
  },
  dismissUpdate: () => {
    set({ updateInfo: null });
  },
  openRelease: async (url: string) => {
    await run(set, "refresh", () => api.openUrl(url));
  },
  loadPrefs: async () => {
    const prefs = await run(set, "load", () => api.getConfig());
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
    await run(set, "snapshot", () => api.snapshotExport(name));
    await useStore.getState().loadSnapshots();
  },
  snapshotImport: async (name: string) => {
    await run(set, "snapshot", () => api.snapshotImport(name));
    await useStore.getState().refreshAll();
  },
  snapshotDelete: async (name: string) => {
    await run(set, "snapshot", () => api.snapshotDelete(name));
    await useStore.getState().loadSnapshots();
  },
  loadSnapshots: async () => {
    const snapshots = await run(set, "load", () => api.snapshotList());
    set({ snapshots });
  },
}));
