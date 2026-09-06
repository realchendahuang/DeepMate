import { Play, Cloud, Puzzle, type LucideIcon } from "lucide-react";

// The app is tenant-first: the outermost rail switches scenarios, the main
// sidebar navigates *inside* the selected scenario (run / providers /
// plugins), and the settings pages at the bottom are system-wide. Switching
// a scenario keeps the current section — the sidebar follows the tenant.
export type View =
  | "scenarios"
  | "run"
  | "providers"
  | "plugins"
  | "diagnostics"
  | "snapshots"
  | "preferences"
  | "about";

export interface NavItemDef {
  view: View;
  labelKey: string;
  icon: LucideIcon;
}


// The per-scenario sections, rendered under the current scenario's name.
export const SCENARIO_SECTIONS: NavItemDef[] = [
  { view: "run", labelKey: "nav.run", icon: Play },
  { view: "providers", labelKey: "nav.providers", icon: Cloud },
  { view: "plugins", labelKey: "nav.plugins", icon: Puzzle },
];
// System-wide pages live behind the rail's settings gear (bottom-left);
// the main sidebar is exclusively the selected scenario's sections.