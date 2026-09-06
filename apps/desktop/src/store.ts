// Global UI state, mirroring the old Slint bridge's UiEvent surface. A single
// zustand store holds the overview, the inventory lists, and the busy/error
// flags so every page reads from one source of truth.

import { create } from "zustand";
import { toast } from "sonner";
import i18n from "./i18n";
import { api } from "./api";
import { mapError } from "./lib/errors";
import type {
  DisabledPlugin,
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
  RuntimeInstance,
  Surface,
  UpdateInfo,
} from "./api";

// Named operations for the busy tracker. Read-only operations (loads, search,
// doctor) only disable their own trigger; mutating operations disable
// everything that would conflict with them.
export type BusyAction =
  | "refresh"
  | "runtime"
  | "doctor"
  | "doctor-fix"
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
  // A doctor fix mutates the profile (installs/removes/updates plugins or
  // starts the runtime), so it must exclude other mutating operations.
  "doctor-fix",
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
  // Every scenario's runtime state (per-scenario instances).
  instances: RuntimeInstance[];
  // The scenario the UI is currently focused on (drives the scenario home
  // and its per-scene providers/models/plugins). Defaults to `web`.
  selectedScenario: string;
  // Plugins the user disabled: uninstalled but remembered, so the installed
  // list can offer a one-click re-enable.
  disabledPlugins: DisabledPlugin[];
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
  setSelectedScenario: (profile: string) => void;
  refreshAll: () => Promise<void>;
  runtimeStart: (profile: string) => Promise<void>;
  runtimeStop: (profile: string) => Promise<void>;
  runtimeRestart: (profile: string) => Promise<void>;
  openHarness: (profile: string) => Promise<void>;
  loadInstances: () => Promise<void>;
  runDoctor: () => Promise<void>;
  fixDoctor: (checkId: string, mode: string) => Promise<void>;
  loadProfiles: () => Promise<void>;
  loadProviders: (profile: string) => Promise<void>;
  loadModels: (profile: string) => Promise<void>;
  upsertProvider: (profile: string, provider: Provider) => Promise<void>;
  removeProvider: (profile: string, id: string) => Promise<void>;
  upsertModel: (profile: string, provider: string, model: Model) => Promise<void>;
  removeModel: (profile: string, provider: string, id: string) => Promise<void>;
  createProfile: (name: string) => Promise<void>;
  createScenario: (name: string, surface: Surface) => Promise<void>;
  removeProfile: (name: string) => Promise<void>;
  renameProfile: (old: string, newName: string) => Promise<void>;
  loadPlugins: () => Promise<void>;
  loadDisabledPlugins: () => Promise<void>;
  // Toggle a plugin: disabling uninstalls it (with its spec remembered for a
  // later re-enable), enabling reinstalls the remembered spec.
  setPluginEnabled: (profile: string, id: string, enabled: boolean) => Promise<void>;
  // Forget a disabled-plugin record without reinstalling it.
  forgetPlugin: (profile: string, id: string) => Promise<void>;
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
  openExternal: (url: string) => Promise<void>;
  configExport: () => Promise<void>;
  configImport: () => Promise<void>;
  loadPrefs: () => Promise<void>;
  snapshotExport: (name: string) => Promise<void>;
  snapshotImport: (name: string) => Promise<void>;
  snapshotDelete: (name: string) => Promise<void>;
  loadSnapshots: () => Promise<void>;
}

// Wrap a command with busy/error handling. Failures surface as a toast with
// a friendly, localized message; the raw error is rethrown for callers that
// need it.
async function run<T>(
  set: (partial: Partial<AppState>) => void,
  action: BusyAction,
  fn: () => Promise<T>,
): Promise<T> {
  set({ busyAction: action });
  try {
    return await fn();
  } catch (e) {
    toast.error(i18n.t(`common.errors.${mapError(e)}`));
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
  instances: [],
  selectedScenario: "web",
  disabledPlugins: [],
  marketSources: [],
  marketEntries: [],
  doctor: null,
  snapshots: [],
  busyAction: null,
  language: "zh",
  theme: "system",
  checkUpdates: true,
  notifyUpdates: true,
  closeToTray: true,
  autostart: false,
  updateInfo: null,
  updateChecked: false,
  opLog: [],
  opActive: false,

  setSelectedScenario: (profile: string) => {
    set({ selectedScenario: profile });
  },
  refreshAll: async () => {
    const overview = await run(set, "refresh", () => api.refreshAll());
    set({ overview });
    // Diagnostics is best-effort during every refresh (including after
    // runtime start/stop) so the Overview health cards stay current; a failed
    // probe keeps the previous report instead of surfacing a toast.
    try {
      set({ doctor: await api.runDoctor() });
    } catch {
      // keep the previous report
    }
  },
  runtimeStart: async (profile: string) => {
    await run(set, "runtime", () => api.runtimeStart(profile));
    await Promise.all([useStore.getState().refreshAll(), useStore.getState().loadInstances()]);
  },
  runtimeStop: async (profile: string) => {
    await run(set, "runtime", () => api.runtimeStop(profile));
    await Promise.all([useStore.getState().refreshAll(), useStore.getState().loadInstances()]);
  },
  runtimeRestart: async (profile: string) => {
    await run(set, "runtime", () => api.runtimeRestart(profile));
    await Promise.all([useStore.getState().refreshAll(), useStore.getState().loadInstances()]);
  },
  openHarness: async (profile: string) => {
    await run(set, "refresh", () => api.openHarness(profile));
  },
  loadInstances: async () => {
    const instances = await run(set, "load", () => api.runtimeList());
    set({ instances });
  },
  runDoctor: async () => {
    const doctor = await run(set, "doctor", () => api.runDoctor());
    set({ doctor });
  },
  // One-click repair for a failing doctor check: clear stale declarations,
  // reinstall/update affected plugins, or start the console. The result is
  // toasted and the report is re-run so the fixed check flips to green.
  fixDoctor: async (checkId: string, mode: string) => {
    const report = await run(set, "doctor-fix", () => api.doctorFix(checkId, mode));
    const key =
      mode === "clear"
        ? "doctor.fixCleared"
        : mode === "start"
          ? "doctor.fixStarted"
          : "doctor.fixReinstalled";
    toast.success(i18n.t(key, { count: report.fixed }));
    await useStore.getState().runDoctor();
  },
  loadProfiles: async () => {
    const profiles = await run(set, "load", () => api.listProfiles());
    set({ profiles });
  },
  loadProviders: async (profile: string) => {
    const providers = await run(set, "load", () => api.listProviders(profile));
    set({ providers });
  },
  loadModels: async (profile: string) => {
    const models = await run(set, "load", () => api.listModels(profile));
    set({ models });
  },
  upsertProvider: async (profile: string, provider: Provider) => {
    await run(set, "save", () => api.upsertProvider(profile, provider));
    await useStore.getState().loadProviders(profile);
  },
  removeProvider: async (profile: string, id: string) => {
    await run(set, "remove", () => api.removeProvider(profile, id));
    await useStore.getState().loadProviders(profile);
    await useStore.getState().loadModels(profile);
  },
  upsertModel: async (profile: string, provider: string, model: Model) => {
    await run(set, "save", () => api.upsertModel(profile, provider, model));
    await useStore.getState().loadModels(profile);
  },
  removeModel: async (profile: string, provider: string, id: string) => {
    await run(set, "remove", () => api.removeModel(profile, provider, id));
    await useStore.getState().loadModels(profile);
  },
  createProfile: async (name: string) => {
    await run(set, "save", () => api.createProfile(name));
    await useStore.getState().loadProfiles();
  },
  // Create a scenario with its surface bundles bootstrapped (web-app or
  // headless), so the new scenario is actually runnable.
  createScenario: async (name: string, surface: Surface) => {
    await run(set, "save", () => api.createScenario(name, surface));
    await Promise.all([
      useStore.getState().loadProfiles(),
      useStore.getState().loadInstances(),
    ]);
  },
  removeProfile: async (name: string) => {
    await run(set, "remove", () => api.removeProfile(name));
    await useStore.getState().loadProfiles();
  },
  renameProfile: async (old: string, newName: string) => {
    await run(set, "save", () => api.renameProfile(old, newName));
    await useStore.getState().loadProfiles();
  },
  loadPlugins: async () => {
    const plugins = await run(set, "load", () => api.listPlugins());
    set({ plugins });
  },
  loadDisabledPlugins: async () => {
    const disabledPlugins = await run(set, "load", () => api.listDisabledPlugins());
    set({ disabledPlugins });
  },
  // Enabling installs a package (busy "install"); disabling removes one
  // (busy "remove"). Both reload the plugin lists together, since a toggle
  // moves a row between them.
  setPluginEnabled: async (profile: string, id: string, enabled: boolean) => {
    if (enabled) {
      await run(set, "install", () => api.pluginEnable(profile, id));
    } else {
      await run(set, "remove", () => api.pluginDisable(profile, id));
    }
    await Promise.all([
      useStore.getState().loadPlugins(),
      useStore.getState().loadDisabledPlugins(),
    ]);
  },
  forgetPlugin: async (profile: string, id: string) => {
    await run(set, "remove", () => api.pluginForget(profile, id));
    await useStore.getState().loadDisabledPlugins();
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
    // Installing a previously disabled plugin clears its registry record.
    await useStore.getState().loadDisabledPlugins();
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
    await useStore.getState().loadDisabledPlugins();
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
      // An install may re-enable a previously disabled plugin.
      await useStore.getState().loadDisabledPlugins();
      const doneKeys = {
        install: "plugins.op.installed",
        remove: "plugins.op.removed",
        update: "plugins.op.updated",
        task: "settings.taskSuccess",
      } as const;
      toast.success(i18n.t(doneKeys[kind], { target }));
    } catch (e) {
      toast.error(i18n.t(`common.errors.${mapError(e)}`));
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
  // Open an external URL (e.g. a plugin's repository) in the system browser.
  openExternal: async (url: string) => {
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
