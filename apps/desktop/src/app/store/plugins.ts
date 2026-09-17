// Plugins across all scenarios plus the marketplace and the live operation
// log. The plugin list is global (the harness reports every scenario's
// plugins in one call); components filter by scenario id.

import { create } from "zustand";
import { toast } from "sonner";
import i18n from "@/i18n";
import { mapError } from "@/shared/lib/errors";
import type {
  DisabledPlugin,
  MarketEntry,
  Plugin,
  PluginOpEvent,
  PluginOpKind,
} from "@/shared/api/api";
import { api } from "@/shared/api/api";
import { runBusy } from "./busy";

interface PluginState {
  plugins: Plugin[];
  disabledPlugins: DisabledPlugin[];
  pluginsLoaded: boolean;
  marketEntries: MarketEntry[];
  // Search state: a monotonic request id so a slow answer cannot overwrite a
  // newer one (switching source or query while a search is in flight), and
  // the failure of the last search so the page can distinguish "no results"
  // from "the search did not run".
  marketSeq: number;
  marketSearching: boolean;
  marketError: string | null;
  // Live plugin operation (install / remove / update): the event log the
  // progress strip renders, and whether an operation is in flight.
  opLog: PluginOpEvent[];
  opActive: boolean;
  loadPlugins: () => Promise<void>;
  loadDisabledPlugins: () => Promise<void>;
  // Toggle a plugin: disabling uninstalls it (with its spec remembered for a
  // later re-enable), enabling reinstalls the remembered spec.
  setPluginEnabled: (profile: string, id: string, enabled: boolean) => Promise<void>;
  searchMarket: (query: string) => Promise<void>;
  // Clear the market result list (used when switching to the community
  // source whose search starts empty).
  resetMarket: () => void;
  // Install from the market with a compatibility preflight: a definite
  // "incompatible" verdict refuses the install; unknown or a failed check
  // lets it proceed (mirroring the CLI's --force-free default).
  marketInstall: (profile: string, spec: string) => Promise<void>;
  // Run a plugin operation with a live progress log. Events append to
  // `opLog` as they arrive; the strip stays open until the caller dismisses.
  runPluginOp: (profile: string, kind: PluginOpKind, target: string) => Promise<void>;
  // Close the progress strip and clear its log.
  dismissPluginOp: () => void;
}

export const usePluginStore = create<PluginState>((set, get) => ({
  plugins: [],
  disabledPlugins: [],
  pluginsLoaded: false,
  marketEntries: [],
  marketSeq: 0,
  marketSearching: false,
  marketError: null,
  opLog: [],
  opActive: false,
  loadPlugins: async () => {
    const plugins = await runBusy("load", () => api.listPlugins());
    set({ plugins, pluginsLoaded: true });
  },
  loadDisabledPlugins: async () => {
    const disabledPlugins = await runBusy("load", () => api.listDisabledPlugins());
    set({ disabledPlugins });
  },
  // Enabling installs a package (busy "install"); disabling removes one
  // (busy "remove"). Both reload the plugin lists together, since a toggle
  // moves a row between them.
  setPluginEnabled: async (profile: string, id: string, enabled: boolean) => {
    if (enabled) {
      await runBusy("install", () => api.pluginEnable(profile, id));
    } else {
      await runBusy("remove", () => api.pluginDisable(profile, id));
    }
    await Promise.all([get().loadPlugins(), get().loadDisabledPlugins()]);
  },
  searchMarket: async (query: string) => {
    // Only the newest search may write its result: without this, a slow
    // response for a previous query (or the previous market source) landed
    // after the user had moved on and replaced the visible list with data
    // that did not match the current view.
    const seq = get().marketSeq + 1;
    set({ marketSeq: seq, marketSearching: true, marketError: null });
    try {
      const marketEntries = await runBusy("search", () => api.marketSearch(query));
      if (get().marketSeq !== seq) return;
      set({ marketEntries, marketSearching: false });
    } catch (error) {
      if (get().marketSeq !== seq) return;
      set({ marketSearching: false, marketError: String(error) });
      throw error;
    }
  },
  resetMarket: () => {
    // Bumping the sequence also invalidates any in-flight search, so its
    // result cannot repopulate a list the user just cleared.
    set((s) => ({
      marketEntries: [],
      marketError: null,
      marketSearching: false,
      marketSeq: s.marketSeq + 1,
    }));
  },
  marketInstall: async (profile: string, spec: string) => {
    await runBusy("install", async () => {
      const report = await api.pluginCheck(spec);
      if (report.status === "incompatible") {
        throw new Error(report.message);
      }
      return api.pluginInstall(profile, spec);
    });
    await Promise.all([get().loadPlugins(), get().loadDisabledPlugins()]);
  },
  runPluginOp: async (profile: string, kind: PluginOpKind, target: string) => {
    set({ opLog: [], opActive: true });
    try {
      await api.pluginOpStream(profile, kind, target, (event) => {
        set((state) => ({ opLog: [...state.opLog, event] }));
      });
      // An install may re-enable a previously disabled plugin.
      await Promise.all([get().loadPlugins(), get().loadDisabledPlugins()]);
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
}));
