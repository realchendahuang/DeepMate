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
  MarketSourceInfo,
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
  marketSources: MarketSourceInfo[];
  marketEntries: MarketEntry[];
  // Live plugin operation (install / remove / update): the event log the
  // progress strip renders, and whether an operation is in flight.
  opLog: PluginOpEvent[];
  opActive: boolean;
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
  marketSources: [],
  marketEntries: [],
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
  forgetPlugin: async (profile: string, id: string) => {
    await runBusy("remove", () => api.pluginForget(profile, id));
    await get().loadDisabledPlugins();
  },
  loadMarketSources: async () => {
    const marketSources = await runBusy("load", () => api.listMarketSources());
    set({ marketSources });
  },
  searchMarket: async (query: string) => {
    const marketEntries = await runBusy("search", () => api.marketSearch(query));
    set({ marketEntries });
  },
  installPlugin: async (profile: string, spec: string) => {
    await runBusy("install", () => api.pluginInstall(profile, spec));
    // Installing a previously disabled plugin clears its registry record.
    await Promise.all([get().loadPlugins(), get().loadDisabledPlugins()]);
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
  removePlugin: async (profile: string, id: string) => {
    await runBusy("remove", () => api.pluginRemove(profile, id));
    await get().loadPlugins();
  },
  updatePlugin: async (profile: string, id: string) => {
    await runBusy("update", () => api.pluginUpdate(profile, id));
    await get().loadPlugins();
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
