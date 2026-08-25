// Thin typed wrappers over the Tauri commands. Each maps to a command in
// src-tauri/src/commands.rs. Errors surface as thrown strings.

import { invoke } from "@tauri-apps/api/core";
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

export const api = {
  refreshAll: () => invoke<Overview>("refresh_all"),
  runtimeStart: () => invoke<void>("runtime_start"),
  runtimeStop: () => invoke<void>("runtime_stop"),
  runtimeRestart: () => invoke<void>("runtime_restart"),
  openHarness: () => invoke<void>("open_harness"),
  runDoctor: () => invoke<DoctorReport>("run_doctor"),
  listProfiles: () => invoke<Profile[]>("list_profiles"),
  listProviders: () => invoke<Provider[]>("list_providers"),
  listModels: () => invoke<Model[]>("list_models"),
  listPlugins: () => invoke<Plugin[]>("list_plugins"),
  pluginInstall: (profile: string, spec: string) =>
    invoke<void>("plugin_install", { profile, spec }),
  pluginRemove: (profile: string, id: string) =>
    invoke<void>("plugin_remove", { profile, id }),
  pluginUpdate: (profile: string, id: string) =>
    invoke<void>("plugin_update", { profile, id }),
  listMarketSources: () => invoke<MarketSourceInfo[]>("list_market_sources"),
  marketSearch: (query: string) =>
    invoke<MarketEntry[]>("market_search", { query }),
  setLanguage: (language: string) => invoke<void>("set_language", { language }),
  setTheme: (theme: string) => invoke<void>("set_theme", { theme }),
  getConfig: () => invoke<{ language: string; theme: string }>("get_config"),
};
