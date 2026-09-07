// Shared busy tracking. Every store action that talks to the Rust side runs
// through `runBusy`, which sets the global busy action, toasts a localized
// failure and clears the flag. Read-only actions never block anything; two
// mutating actions can never run at the same time.

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

// Mutating operations that must not run concurrently with each other.
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
  // starts the runtime), so it must exclude other mutating operations.
  "doctor-fix",
]);

export function isBlocked(active: BusyAction | null, candidate: BusyAction): boolean {
  if (active === null) return false;
  if (active === candidate) return true;
  return MUTATING.has(active) && MUTATING.has(candidate);
}

export const useBusyStore = create<{ busyAction: BusyAction | null }>(() => ({
  busyAction: null,
}));

// Wrap a command with busy/error handling. Failures surface as a toast with a
// friendly, localized message; the raw error is rethrown for callers that
// need it.
export async function runBusy<T>(action: BusyAction, fn: () => Promise<T>): Promise<T> {
  useBusyStore.setState({ busyAction: action });
  try {
    return await fn();
  } catch (e) {
    toast.error(i18n.t(`common.errors.${mapError(e)}`));
    throw e;
  } finally {
    useBusyStore.setState({ busyAction: null });
  }
}
