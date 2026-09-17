// Per-scenario provider & model catalogues. Presence of a scenario id in
// these maps doubles as the "loaded" flag: switching scenarios renders the
// cached data instantly and only shows a skeleton the first time. Mutations
// reload only the affected scenario's slices.
//
// Failures are recorded too. A load that only toasted and left the map empty
// made the page render a skeleton forever, which reads as "the app hung"
// rather than "this failed, try again".

import { create } from "zustand";
import type { Model, Provider } from "@/shared/api/api";
import { api } from "@/shared/api/api";
import { runBusy } from "./busy";

interface ProviderState {
  providers: Record<string, Provider[]>;
  models: Record<string, Model[]>;
  // Per-scenario load failures, cleared by a successful load.
  providersError: Record<string, string>;
  modelsError: Record<string, string>;
  loadProviders: (profile: string) => Promise<void>;
  loadModels: (profile: string) => Promise<void>;
  upsertProvider: (profile: string, provider: Provider) => Promise<void>;
  removeProvider: (profile: string, id: string) => Promise<void>;
  upsertModel: (profile: string, provider: string, model: Model) => Promise<void>;
  removeModel: (profile: string, provider: string, id: string) => Promise<void>;
}

export const useProviderStore = create<ProviderState>((set, get) => ({
  providers: {},
  models: {},
  providersError: {},
  modelsError: {},
  loadProviders: async (profile: string) => {
    try {
      const providers = await runBusy("load", () => api.listProviders(profile));
      set((s) => {
        const providersError = { ...s.providersError };
        delete providersError[profile];
        return { providers: { ...s.providers, [profile]: providers }, providersError };
      });
    } catch (error) {
      // runBusy already toasted; keep the failure so the page can offer a
      // retry instead of showing a skeleton forever.
      set((s) => ({
        providersError: { ...s.providersError, [profile]: String(error) },
      }));
    }
  },
  loadModels: async (profile: string) => {
    try {
      const models = await runBusy("load", () => api.listModels(profile));
      set((s) => {
        const modelsError = { ...s.modelsError };
        delete modelsError[profile];
        return { models: { ...s.models, [profile]: models }, modelsError };
      });
    } catch (error) {
      set((s) => ({ modelsError: { ...s.modelsError, [profile]: String(error) } }));
    }
  },
  upsertProvider: async (profile: string, provider: Provider) => {
    await runBusy("save", () => api.upsertProvider(profile, provider));
    await get().loadProviders(profile);
  },
  removeProvider: async (profile: string, id: string) => {
    await runBusy("remove", () => api.removeProvider(profile, id));
    await Promise.all([get().loadProviders(profile), get().loadModels(profile)]);
  },
  upsertModel: async (profile: string, provider: string, model: Model) => {
    await runBusy("save", () => api.upsertModel(profile, provider, model));
    await get().loadModels(profile);
  },
  removeModel: async (profile: string, provider: string, id: string) => {
    await runBusy("remove", () => api.removeModel(profile, provider, id));
    await get().loadModels(profile);
  },
}));
