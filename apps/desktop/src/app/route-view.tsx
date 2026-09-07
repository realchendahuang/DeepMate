// Mounts the component for the current route. The route is the only input;
// every view reads its own stores.

import type { Route } from "./router";
import { ScenariosPage } from "@/features/scenarios/scenarios-page";
import { ScenarioSections } from "@/features/scenarios/scenario-sections";
import { SettingsPage } from "@/features/settings/settings-page";

export function RouteView({ route }: { route: Route }) {
  switch (route.kind) {
    case "scenarios":
      return <ScenariosPage />;
    case "scenario":
      return <ScenarioSections section={route.section} />;
    case "settings":
      return <SettingsPage section={route.section} />;
  }
}
