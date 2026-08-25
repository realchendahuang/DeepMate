import { LayoutDashboard, Puzzle, Settings, type LucideIcon } from "lucide-react";

export type View = "overview" | "plugins" | "settings";

export interface NavItemDef {
  view: View;
  labelKey: string;
  icon: LucideIcon;
}

// Primary workspace navigation; settings is pinned to the sidebar footer.
export const NAV_MAIN: NavItemDef[] = [
  { view: "overview", labelKey: "nav.overview", icon: LayoutDashboard },
  { view: "plugins", labelKey: "nav.plugins", icon: Puzzle },
];

export const NAV_FOOTER: NavItemDef[] = [
  { view: "settings", labelKey: "nav.settings", icon: Settings },
];
