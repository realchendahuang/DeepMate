// The section sidebar: either the selected scenario's sections or the system
// settings list — never a mix. On the all-scenarios view it lists the
// scenarios themselves.

import { useTranslation } from "react-i18next";
import {
  ArrowLeft,
  Cloud,
  FileCode2,
  Globe,
  Info,
  LayoutDashboard,
  Layers,
  Puzzle,
  ShieldCheck,
  SlidersHorizontal,
  Store,
  TerminalSquare,
  type LucideIcon,
} from "lucide-react";
import { useRouterStore, type ScenarioSection, type SettingSection } from "../router";
import { NavItem } from "./nav-item";
import { useScenarioStore } from "../store/scenarios";
import { useRuntimeStore } from "../store/runtime";
import { usePluginStore } from "../store/plugins";
import { useProviderStore } from "../store/providers";
import { Button } from "@/shared/ui/button";
import { cn } from "@/shared/lib/utils";
import { scenarioInitial } from "@/shared/lib/scenario";

interface SectionDef<V extends string> {
  view: V;
  labelKey: string;
  icon: LucideIcon;
}

const SCENARIO_SECTIONS: SectionDef<ScenarioSection>[] = [
  { view: "overview", labelKey: "nav.overview", icon: LayoutDashboard },
  { view: "providers", labelKey: "nav.providers", icon: Cloud },
  { view: "plugins", labelKey: "nav.plugins", icon: Puzzle },
];

const SETTING_SECTIONS: SectionDef<SettingSection>[] = [
  { view: "diagnostics", labelKey: "settings.diagnostics", icon: ShieldCheck },
  { view: "snapshots", labelKey: "settings.snapshots", icon: Layers },
  { view: "preferences", labelKey: "settings.preferences", icon: SlidersHorizontal },
  { view: "market", labelKey: "settings.market", icon: Store },
  { view: "advanced", labelKey: "settings.advanced", icon: FileCode2 },
  { view: "about", labelKey: "settings.about", icon: Info },
];

function NavList({ children }: { children: React.ReactNode }) {
  const { t } = useTranslation();
  return (
    <nav aria-label={t("nav.menu")} className="flex-1 space-y-0.5 overflow-y-auto px-2 py-3">
      {children}
    </nav>
  );
}

// The sidebar's nav body, shared with the mobile drawer.
export function SidebarNav({ pillId }: { pillId: string }) {
  const { t } = useTranslation();
  const route = useRouterStore((s) => s.route);
  const navigate = useRouterStore((s) => s.navigate);
  const openScenario = useRouterStore((s) => s.openScenario);
  const scenarios = useScenarioStore((s) => s.profiles);
  const selectedScenario = useScenarioStore((s) => s.selectedScenario);
  const instances = useRuntimeStore((s) => s.instances);
  const plugins = usePluginStore((s) => s.plugins);
  const models = useProviderStore((s) => s.models[selectedScenario]);

  const scenarioPlugins = plugins.filter((p) => p.profile === selectedScenario);
  const hasOutdated = scenarioPlugins.some((p) => p.outdated);
  const modelCount = models?.length ?? 0;

  if (route.kind === "scenarios") {
    return (
      <nav
        aria-label={t("nav.menu")}
        className="flex-1 space-y-1 overflow-y-auto px-2 py-3"
      >
        {scenarios.map((profile) => {
          const instance = instances.find((item) => item.profile === profile.id);
          const running = instance?.status === "running";
          const active = selectedScenario === profile.id;
          return (
            <button
              key={profile.id}
              type="button"
              onClick={() => openScenario(profile.id)}
              className={cn(
                "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-body transition-colors",
                active ? "bg-hover font-semibold text-text" : "text-text-dim hover:bg-hover/60",
              )}
            >
              <span
                className={cn("h-2 w-2 shrink-0 rounded-full", running ? "bg-pass" : "bg-neutral")}
              />
              <span className="min-w-0 flex-1 truncate text-left">{profile.name}</span>
              {instance?.surface === "task" ? (
                <TerminalSquare className="h-3.5 w-3.5 shrink-0 text-text-faint" />
              ) : instance?.surface === "web" ? (
                <Globe className="h-3.5 w-3.5 shrink-0 text-text-faint" />
              ) : null}
            </button>
          );
        })}
        {scenarios.length === 0 && (
          <div className="px-2 text-caption text-text-faint">{t("overview.noScenarios")}</div>
        )}
      </nav>
    );
  }

  if (route.kind === "settings") {
    return (
      <NavList>
        {SETTING_SECTIONS.map((item) => (
          <NavItem
            key={item.view}
            icon={item.icon}
            labelKey={item.labelKey}
            active={route.section === item.view}
            onClick={() => navigate({ kind: "settings", section: item.view })}
            pillId={pillId}
          />
        ))}
      </NavList>
    );
  }

  return (
    <NavList>
      {SCENARIO_SECTIONS.map((item) => {
        let badge: React.ReactNode = null;
        if (item.view === "plugins" && scenarioPlugins.length > 0) {
          badge = (
            <span className="flex items-center gap-1">
              {hasOutdated && (
                <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-warn" />
              )}
              <span className="rounded-full bg-panel-2 px-1.5 py-0.5 font-mono text-[10px] font-semibold text-text-dim">
                {scenarioPlugins.length}
              </span>
            </span>
          );
        } else if (item.view === "providers" && modelCount > 0) {
          badge = (
            <span className="rounded-full bg-panel-2 px-1.5 py-0.5 font-mono text-[10px] font-semibold text-text-dim">
              {modelCount}
            </span>
          );
        }

        return (
          <NavItem
            key={item.view}
            icon={item.icon}
            labelKey={item.labelKey}
            active={route.section === item.view}
            onClick={() => navigate({ kind: "scenario", section: item.view })}
            pillId={pillId}
            badge={badge}
          />
        );
      })}
    </NavList>
  );
}

function SidebarHeader() {
  const { t } = useTranslation();
  const route = useRouterStore((s) => s.route);
  const openScenario = useRouterStore((s) => s.openScenario);
  const selectedScenario = useScenarioStore((s) => s.selectedScenario);
  const scenarios = useScenarioStore((s) => s.profiles);
  const instances = useRuntimeStore((s) => s.instances);

  if (route.kind === "settings") {
    const activeProfile = scenarios.find((p) => p.id === selectedScenario);
    return (
      <div className="flex h-[52px] shrink-0 items-center justify-between border-b border-border px-3">
        <span className="truncate text-heading font-semibold text-text">
          {t("settings.title")}
        </span>
        {selectedScenario && (
          <Button
            variant="ghost"
            size="sm"
            onClick={() => openScenario(selectedScenario)}
            className="h-7 gap-1 px-2 text-small text-text-dim hover:text-text"
            title={t("nav.backToScenario")}
          >
            <ArrowLeft className="h-3.5 w-3.5" />
            <span className="max-w-[80px] truncate text-small">
              {activeProfile?.name ?? selectedScenario}
            </span>
          </Button>
        )}
      </div>
    );
  }

  let title = t("settings.title");
  let scenario = false;
  if (route.kind === "scenarios") {
    title = t("settings.scenarios");
  } else if (route.kind === "scenario") {
    title = scenarios.find((profile) => profile.id === selectedScenario)?.name ?? selectedScenario;
    scenario = true;
  }
  const running =
    instances.find((item) => item.profile === selectedScenario)?.status === "running";

  return (
    <div className="flex h-[52px] shrink-0 items-center gap-2.5 border-b border-border px-4">
      {scenario ? (
        <>
          <div
            className={cn(
              "flex h-6 w-6 shrink-0 items-center justify-center rounded-md text-[11px] font-bold",
              running ? "bg-accent text-on-accent shadow-sm" : "bg-panel-2 text-text-dim",
            )}
          >
            {scenarioInitial(title)}
          </div>
          <span className="min-w-0 flex-1 truncate text-heading font-semibold text-text">
            {title}
          </span>
          <span
            className={cn(
              "relative flex h-2 w-2 shrink-0 items-center justify-center rounded-full",
              running ? "bg-pass" : "bg-neutral",
            )}
          >
            {running && (
              <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-pass opacity-75" />
            )}
          </span>
        </>
      ) : (
        <span className="min-w-0 truncate text-heading font-semibold text-text">
          {title || t("settings.title")}
        </span>
      )}
    </div>
  );
}

export function AppSidebar() {
  return (
    <aside className="hidden w-sidebar shrink-0 flex-col border-r border-border bg-sidebar md:flex">
      <SidebarHeader />
      <SidebarNav pillId="sidebar-nav" />
    </aside>
  );
}
