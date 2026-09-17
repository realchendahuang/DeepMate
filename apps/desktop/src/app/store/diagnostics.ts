// The doctor report. Loaded on demand by the diagnostics page — the engine
// overview refresh no longer runs a full doctor pass every poll, since
// nothing else renders the report.

import { create } from "zustand";
import { toast } from "sonner";
import i18n from "@/i18n";
import type { DoctorReport } from "@/shared/api/api";
import { api } from "@/shared/api/api";
import { runBusy } from "./busy";

interface DiagnosticsState {
  doctor: DoctorReport | null;
  runDoctor: () => Promise<void>;
  // One-click repair for a failing doctor check: clear stale declarations,
  // reinstall/update affected plugins, or start the console. The result is
  // toasted and the report is re-run so the fixed check flips to green.
  fixDoctor: (checkId: string, mode: string) => Promise<void>;
}

export const useDiagnosticsStore = create<DiagnosticsState>((set, get) => ({
  doctor: null,
  runDoctor: async () => {
    try {
      const doctor = await runBusy("doctor", () => api.runDoctor());
      set({ doctor });
    } catch {
      // The page keeps the previous report; the failure was already toasted.
      // Leaving the stale report visible is better than blanking the page,
      // and the report's own timestamp tells the user how fresh it is.
    }
  },
  fixDoctor: async (checkId: string, mode: string) => {
    const report = await runBusy("doctor-fix", () => api.doctorFix(checkId, mode));
    const key =
      mode === "clear"
        ? "doctor.fixCleared"
        : mode === "start"
          ? "doctor.fixStarted"
          : "doctor.fixReinstalled";
    // A partial repair is a real outcome: saying "repaired 3" while two items
    // are still broken sends the user away with a false sense of health.
    if (report.failures.length > 0) {
      toast.warning(
        i18n.t("common.errors.doctorPartlyFixed", {
          fixed: report.fixed,
          failed: report.failures.length,
        }),
      );
    } else {
      toast.success(i18n.t(key, { count: report.fixed }));
    }
    // Re-run the report either way: the repair changed state, so the shown
    // report must reflect it (including a repair that only partly worked).
    await get().runDoctor();
  },
}));
