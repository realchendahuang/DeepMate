// Configuration snapshots: export / import / delete the app state bundle.

import { create } from "zustand";
import { api } from "@/shared/api/api";
import { runBusy } from "./busy";
import { useRuntimeStore } from "./runtime";

interface SnapshotState {
  snapshots: string[];
  snapshotsLoaded: boolean;
  loadSnapshots: () => Promise<void>;
  snapshotExport: (name: string) => Promise<void>;
  snapshotImport: (name: string) => Promise<void>;
  snapshotDelete: (name: string) => Promise<void>;
}

export const useSnapshotStore = create<SnapshotState>((set, get) => ({
  snapshots: [],
  snapshotsLoaded: false,
  loadSnapshots: async () => {
    const snapshots = await runBusy("load", () => api.snapshotList());
    set({ snapshots, snapshotsLoaded: true });
  },
  snapshotExport: async (name: string) => {
    await runBusy("snapshot", () => api.snapshotExport(name));
    await get().loadSnapshots();
  },
  snapshotImport: async (name: string) => {
    await runBusy("snapshot", () => api.snapshotImport(name));
    await useRuntimeStore.getState().refreshOverview();
  },
  snapshotDelete: async (name: string) => {
    await runBusy("snapshot", () => api.snapshotDelete(name));
    await get().loadSnapshots();
  },
}));
