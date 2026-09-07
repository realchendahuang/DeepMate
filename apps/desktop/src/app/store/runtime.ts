// Engine + runtime state: the engine detection report, every scenario's
// running instance, and the one-shot task runs. All of this is global —
// instances and tasks are keyed by scenario id, so switching scenarios never
// loses a running task's log.

import { create } from "zustand";
import type { Overview, RuntimeInstance } from "@/shared/api/api";
import { api } from "@/shared/api/api";
import { runBusy } from "./busy";

export interface TaskRun {
  running: boolean;
  done: boolean;
  failed: boolean;
  lines: string[];
}

const IDLE_TASK: TaskRun = { running: false, done: false, failed: false, lines: [] };

interface RuntimeState {
  instances: RuntimeInstance[];
  instancesLoaded: boolean;
  overview: Overview | null;
  tasks: Record<string, TaskRun>;
  loadInstances: () => Promise<void>;
  refreshOverview: () => Promise<void>;
  runtimeStart: (profile: string) => Promise<void>;
  runtimeStop: (profile: string) => Promise<void>;
  runtimeRestart: (profile: string) => Promise<void>;
  openHarness: (profile: string) => Promise<void>;
  runTask: (profile: string, prompt: string) => Promise<void>;
}

export const useRuntimeStore = create<RuntimeState>((set, get) => ({
  instances: [],
  instancesLoaded: false,
  overview: null,
  tasks: {},
  loadInstances: async () => {
    const instances = await runBusy("load", () => api.runtimeList());
    set({ instances, instancesLoaded: true });
  },
  refreshOverview: async () => {
    const overview = await runBusy("refresh", () => api.refreshOverview());
    set({ overview });
  },
  runtimeStart: async (profile: string) => {
    await runBusy("runtime", () => api.runtimeStart(profile));
    await Promise.all([get().refreshOverview(), get().loadInstances()]);
  },
  runtimeStop: async (profile: string) => {
    await runBusy("runtime", () => api.runtimeStop(profile));
    await Promise.all([get().refreshOverview(), get().loadInstances()]);
  },
  runtimeRestart: async (profile: string) => {
    await runBusy("runtime", () => api.runtimeRestart(profile));
    await Promise.all([get().refreshOverview(), get().loadInstances()]);
  },
  openHarness: async (profile: string) => {
    await runBusy("refresh", () => api.openHarness(profile));
  },
  runTask: async (profile: string, prompt: string) => {
    set((s) => ({
      tasks: { ...s.tasks, [profile]: { running: true, done: false, failed: false, lines: [] } },
    }));
    try {
      await api.taskRun(profile, prompt, (event) => {
        set((s) => {
          const task = s.tasks[profile] ?? IDLE_TASK;
          if (event.phase === "line") {
            return { tasks: { ...s.tasks, [profile]: { ...task, lines: [...task.lines, event.text] } } };
          }
          if (event.phase === "finished") {
            return {
              tasks: { ...s.tasks, [profile]: { ...task, running: false, done: true, failed: !event.ok } },
            };
          }
          return s;
        });
      });
    } catch {
      // The failed state is enough: the task card renders the failure icon
      // and keeps the partial log, matching a finished-with-error run.
      set((s) => {
        const task = s.tasks[profile] ?? IDLE_TASK;
        return { tasks: { ...s.tasks, [profile]: { ...task, running: false, done: true, failed: true } } };
      });
    }
  },
}));
