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
    const doctor = await runBusy("doctor", () => api.runDoctor());
    set({ doctor });
  },
  fixDoctor: async (checkId: string, mode: string) => {
    const report = await runBusy("doctor-fix", () => api.doctorFix(checkId, mode));
    const key =
      mode === "clear"
        ? "doctor.fixCleared"
        : mode === "start"
          ? "doctor.fixStarted"
          : "doctor.fixReinstalled";
    toast.success(i18n.t(key, { count: report.fixed }));
    await get().runDoctor();
  },
}));
