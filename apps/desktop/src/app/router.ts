// The app's single navigation state. A route is always one of: the
// all-scenarios inventory, a section of the selected scenario, or a system
// settings section. Every chrome component reads this store directly instead
// of threading callbacks; only the shell mounts views from it.

import { create } from "zustand";
import { useScenarioStore } from "./store/scenarios";

export type ScenarioSection = "overview" | "providers" | "plugins";
export type SettingSection = "diagnostics" | "snapshots" | "preferences" | "about";

export type Route =
  | { kind: "scenarios" }
  | { kind: "scenario"; section: ScenarioSection }
  | { kind: "settings"; section: SettingSection };

// A scenario's overview is the app's landing view.
export const SCENARIO_HOME: Route = { kind: "scenario", section: "overview" };

export function routeKey(route: Route): string {
  return route.kind === "scenarios" ? route.kind : `${route.kind}/${route.section}`;
}

// Flat order used only for the slide direction of page transitions.
const ROUTE_ORDER: readonly string[] = [
  "scenarios",
  "scenario/overview",
  "scenario/providers",
  "scenario/plugins",
  "settings/diagnostics",
  "settings/snapshots",
  "settings/preferences",
  "settings/about",
];

export function routeIndex(route: Route): number {
  return ROUTE_ORDER.indexOf(routeKey(route));
}

interface RouterState {
  route: Route;
  // Slide direction of the page transition for the most recent move.
  direction: 1 | -1;
  navigate: (route: Route) => void;
  // Switch the focused scenario. A scenario section is kept as-is; anywhere
  // else (settings / all-scenarios) hands over to the new scenario's home.
  openScenario: (profileId: string) => void;
  openSettings: (section: SettingSection) => void;
  openAllScenarios: () => void;
}

export const useRouterStore = create<RouterState>((set, get) => {
  const move = (route: Route) => {
    const from = get().route;
    set({
      route,
      direction: routeIndex(route) >= routeIndex(from) ? 1 : -1,
    });
  };

  return {
    route: SCENARIO_HOME,
    direction: 1,
    navigate: move,
    openScenario: (profileId: string) => {
      useScenarioStore.getState().setSelectedScenario(profileId);
      if (get().route.kind !== "scenario") move(SCENARIO_HOME);
    },
    openSettings: (section: SettingSection) => move({ kind: "settings", section }),
    openAllScenarios: () => move({ kind: "scenarios" }),
  };
});
