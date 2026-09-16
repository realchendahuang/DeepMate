// The scenario inventory plus the scenario the UI is focused on. Scenario
// CRUD keeps the cross-store invariants in one place: creating selects the
// new scenario, renaming moves the selection with the id (rename moves the
// profile directory, so the id changes), and deleting the selected scenario
// falls back to the first remaining one.

import { create } from "zustand";
import type { Profile, Surface } from "@/shared/api/api";
import { api } from "@/shared/api/api";
import { DEFAULT_SCENARIO_ID } from "@/shared/lib/scenario";
import { runBusy } from "./busy";
import { useRuntimeStore } from "./runtime";

interface ScenarioState {
  profiles: Profile[];
  profilesLoaded: boolean;
  selectedScenario: string;
  setSelectedScenario: (profile: string) => void;
  loadProfiles: () => Promise<void>;
  createScenario: (name: string, surface: Surface) => Promise<void>;
  renameScenario: (oldId: string, newName: string) => Promise<void>;
  removeScenario: (profile: string) => Promise<void>;
  updateDescription: (profile: string, description: string) => Promise<void>;
}

export const useScenarioStore = create<ScenarioState>((set, get) => ({
  profiles: [],
  profilesLoaded: false,
  selectedScenario: DEFAULT_SCENARIO_ID,
  setSelectedScenario: (profile: string) => {
    set({ selectedScenario: profile });
  },
  loadProfiles: async () => {
    const profiles = await runBusy("load", () => api.listScenarios());
    set({ profiles, profilesLoaded: true });
  },
  // Create a scenario with its surface bundles bootstrapped (web-app or
  // headless), select it and refresh the running instances so the new
  // scenario is actually runnable.
  createScenario: async (name: string, surface: Surface) => {
    await runBusy("save", () => api.createScenario(name, surface));
    set({ selectedScenario: name });
    await Promise.all([get().loadProfiles(), useRuntimeStore.getState().loadInstances()]);
  },
  renameScenario: async (oldId: string, newName: string) => {
    await runBusy("save", () => api.renameScenario(oldId, newName));
    await get().loadProfiles();
    if (get().selectedScenario === oldId) {
      set({ selectedScenario: newName });
    }
  },
  removeScenario: async (profile: string) => {
    await runBusy("remove", () => api.removeScenario(profile));
    await get().loadProfiles();
    if (get().selectedScenario === profile || get().selectedScenario === "") {
      set({ selectedScenario: get().profiles[0]?.id ?? "" });
    }
  },
  // Set a scenario's description; an empty text clears it (the engine's
  // bundles fallback description takes over again).
  updateDescription: async (profile: string, description: string) => {
    await runBusy("save", () =>
      api.setScenarioDescription(profile, description.trim() ? description.trim() : null),
    );
    await get().loadProfiles();
  },
}));
