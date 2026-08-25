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
  // UI.
  busy: boolean;
  error: string | null;
  language: string;
  theme: string;

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
  loadPlugins: () => Promise<void>;
  loadMarketSources: () => Promise<void>;
  searchMarket: (query: string) => Promise<void>;
  installPlugin: (profile: string, spec: string) => Promise<void>;
  removePlugin: (profile: string, id: string) => Promise<void>;
  updatePlugin: (profile: string, id: string) => Promise<void>;
  setLanguage: (language: string) => Promise<void>;
  setTheme: (theme: string) => Promise<void>;
  loadPrefs: () => Promise<void>;
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
  busy: false,
  error: null,
  language: "en",
  theme: "system",

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
  loadPrefs: async () => {
    const prefs = await run(set, () => api.getConfig());
    set({ language: prefs.language, theme: prefs.theme });
    (await import("./i18n")).default.changeLanguage(prefs.language);
  },
}));
