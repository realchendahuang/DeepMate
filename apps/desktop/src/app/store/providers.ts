// Per-scenario provider & model catalogues. Presence of a scenario id in
// these maps doubles as the "loaded" flag: switching scenarios renders the
// cached data instantly and only shows a skeleton the first time. Mutations
// reload only the affected scenario's slices.

import { create } from "zustand";
import type { Model, Provider } from "@/shared/api/api";
import { api } from "@/shared/api/api";
import { runBusy } from "./busy";

interface ProviderState {
  providers: Record<string, Provider[]>;
  models: Record<string, Model[]>;
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
  loadProviders: async (profile: string) => {
    const providers = await runBusy("load", () => api.listProviders(profile));
    set((s) => ({ providers: { ...s.providers, [profile]: providers } }));
  },
  loadModels: async (profile: string) => {
    const models = await runBusy("load", () => api.listModels(profile));
    set((s) => ({ models: { ...s.models, [profile]: models } }));
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
