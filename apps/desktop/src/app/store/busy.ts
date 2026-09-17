// Shared busy tracking. Every store action that talks to the Rust side runs
// through `runBusy`, which records the in-flight action, toasts a localized
// failure and removes it again.
//
// Two invariants matter here:
// - Read-only actions (load / refresh / doctor / search) never disable the
//   UI. The overview polls on a timer, and blocking on every poll made the
//   whole app flicker into a disabled state.
// - Each action tracks itself: concurrent reads cannot clear each other's
//   flag the way a single "current action" slot did.

import { create } from "zustand";
import { toast } from "sonner";
import i18n from "@/i18n";
import { mapError } from "@/shared/lib/errors";

export type BusyAction =
  | "refresh"
  | "runtime"
  | "doctor"
  | "doctor-fix"
  | "load"
  | "search"
  | "install"
  | "remove"
  | "update"
  | "save"
  | "prefs"
  | "update-install"
  | "config"
  | "snapshot";

// Mutating operations that disable the controls while they run.
const MUTATING: ReadonlySet<BusyAction> = new Set([
  "install",
  "remove",
  "update",
  "save",
  "prefs",
  "update-install",
  "config",
  "snapshot",
  // A doctor fix mutates the profile (installs/removes/updates plugins or
  // starts the runtime), so it disables the controls too.
  "doctor-fix",
  // Starting, stopping and restarting a scenario are slow (a boot wait, a
  // stop that has to wait for the process to actually exit). Leaving them out
  // let a user click Start repeatedly while one was already running.
  "runtime",
]);

export function isMutating(action: BusyAction): boolean {
  return MUTATING.has(action);
}

export const useBusyStore = create<{ active: BusyAction[] }>(() => ({ active: [] }));

// Whether a specific action is in flight; drives per-action spinners.
export function useActionActive(action: BusyAction): boolean {
  return useBusyStore((state) => state.active.includes(action));
}

// Whether any mutating action is in flight; drives disabled controls.
export function useBlocking(): boolean {
  return useBusyStore((state) => state.active.some(isMutating));
}

// Wrap a command with busy/error handling. Failures surface as a toast with a
// friendly, localized message; the raw error is rethrown for callers that
// need it.
export async function runBusy<T>(action: BusyAction, fn: () => Promise<T>): Promise<T> {
  useBusyStore.setState((state) => ({ active: [...state.active, action] }));
  try {
    return await fn();
  } catch (e) {
    toast.error(i18n.t(`common.errors.${mapError(e)}`));
    throw e;
  } finally {
    useBusyStore.setState((state) => {
      // Remove one instance of this action (it may have been started twice).
      const index = state.active.indexOf(action);
      if (index === -1) return state;
      return {
        active: [...state.active.slice(0, index), ...state.active.slice(index + 1)],
      };
    });
  }
}
