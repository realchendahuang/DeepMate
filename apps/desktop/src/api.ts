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
  UpdateInfo,
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
  upsertProvider: (provider: Provider) =>
    invoke<void>("upsert_provider", { provider }),
  removeProvider: (id: string) => invoke<void>("remove_provider", { id }),
  upsertModel: (provider: string, model: Model) =>
    invoke<void>("upsert_model", { provider, model }),
  removeModel: (provider: string, id: string) =>
    invoke<void>("remove_model", { provider, id }),
  createProfile: (name: string) =>
    invoke<void>("create_profile", { name }),
  removeProfile: (name: string) =>
    invoke<void>("remove_profile", { name }),
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
  snapshotExport: (name: string) =>
    invoke<void>("snapshot_export", { name }),
  snapshotImport: (name: string) =>
    invoke<void>("snapshot_import", { name }),
  snapshotList: () => invoke<string[]>("snapshot_list"),
  setLanguage: (language: string) => invoke<void>("set_language", { language }),
  setTheme: (theme: string) => invoke<void>("set_theme", { theme }),
  setCloseToTray: (enabled: boolean) =>
    invoke<void>("set_close_to_tray", { enabled }),
  setCheckUpdates: (enabled: boolean) =>
    invoke<void>("set_check_updates", { enabled }),
  autostartGet: () => invoke<boolean>("autostart_get"),
  autostartSet: (enabled: boolean) =>
    invoke<void>("autostart_set", { enabled }),
  checkUpdate: () => invoke<UpdateInfo | null>("check_update"),
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  getConfig: () =>
    invoke<{
      language: string;
      theme: string;
      check_updates: boolean;
      close_to_tray: boolean;
    }>("get_config"),
};
