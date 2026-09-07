// The scenario rail — the outermost column: one avatar per scenario, a
// trailing "+" to create one (which owns its own dialog), a home button
// (all scenarios) on top, and system settings pinned to the bottom. No
// product logo here — the mark is a rounded-square asset and fighting a
// circular rail button made it worse.

import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { LayoutGrid, Plus, Settings } from "lucide-react";
import { useRouterStore, SCENARIO_HOME } from "../router";
import { useScenarioStore } from "../store/scenarios";
import { useRuntimeStore } from "../store/runtime";
import { cn } from "@/shared/lib/utils";
import { scenarioInitial } from "@/shared/lib/scenario";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/shared/ui/tooltip";
import { NewScenarioDialog } from "@/features/scenarios/new-scenario-dialog";

function RailButton({
  active,
  onClick,
  label,
  running,
  children,
  className,
}: {
  active?: boolean;
  onClick: () => void;
  label: string;
  running?: boolean;
  children: ReactNode;
  className?: string;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <div className="group/rail relative flex w-rail shrink-0 justify-center">
          <span
            className={cn(
              "absolute top-1/2 left-0 w-1 -translate-y-1/2 rounded-r-full bg-text transition-all",
              active
                ? "h-10 opacity-100"
                : "h-0 opacity-0 group-hover/rail:h-5 group-hover/rail:opacity-70",
            )}
            aria-hidden
          />
          <button
            type="button"
            onClick={onClick}
            aria-label={label}
            aria-current={active ? "true" : undefined}
            className={cn(
              "relative flex h-12 w-12 shrink-0 items-center justify-center text-small font-bold transition-[border-radius,background-color,color]",
              active
                ? "rounded-2xl bg-accent text-on-accent"
                : "rounded-full bg-panel-2 text-text-dim hover:rounded-2xl hover:bg-hover hover:text-text",
              className,
            )}
          >
            {children}
          </button>
          {running !== undefined && (
            <span
              className={cn(
                "pointer-events-none absolute right-2.5 bottom-0.5 h-3 w-3 rounded-full border-2 border-rail",
                running ? "bg-pass" : "bg-neutral",
              )}
              aria-hidden
            />
          )}
        </div>
      </TooltipTrigger>
      <TooltipContent side="right">{label}</TooltipContent>
    </Tooltip>
  );
}

export function ScenarioRail() {
  const { t } = useTranslation();
  const route = useRouterStore((s) => s.route);
  const navigate = useRouterStore((s) => s.navigate);
  const openScenario = useRouterStore((s) => s.openScenario);
  const openAllScenarios = useRouterStore((s) => s.openAllScenarios);
  const openSettings = useRouterStore((s) => s.openSettings);
  const profiles = useScenarioStore((s) => s.profiles);
  const selectedScenario = useScenarioStore((s) => s.selectedScenario);
  const instances = useRuntimeStore((s) => s.instances);
  const [creating, setCreating] = useState(false);

  const allActive = route.kind === "scenarios";
  const settingsActive = route.kind === "settings";
  const sceneActive = route.kind === "scenario";

  const scenes = (
    <>
      {profiles.map((profile) => {
        const instance = instances.find((item) => item.profile === profile.id);
        const running = instance?.status === "running";
        const active = sceneActive && selectedScenario === profile.id;
        const label =
          profile.id === "web"
            ? t("scene.defaultName", { name: profile.name })
            : profile.name;
        return (
          <RailButton
            key={profile.id}
            active={active}
            running={running}
            label={label}
            onClick={() => openScenario(profile.id)}
          >
            {scenarioInitial(profile.name)}
          </RailButton>
        );
      })}
      <Tooltip>
        <TooltipTrigger asChild>
          <div className="relative flex w-rail shrink-0 justify-center">
            <button
              type="button"
              onClick={() => setCreating(true)}
              aria-label={t("settings.addProfile")}
              title={t("settings.addProfile")}
              className="flex h-12 w-12 shrink-0 items-center justify-center rounded-full bg-panel-2 text-text-faint transition-[border-radius,background-color,color] hover:rounded-2xl hover:bg-pass/15 hover:text-pass"
            >
              <Plus className="h-5 w-5" />
            </button>
          </div>
        </TooltipTrigger>
        <TooltipContent side="right">{t("settings.addProfile")}</TooltipContent>
      </Tooltip>
    </>
  );

  return (
    <>
      <aside className="hidden h-full w-rail shrink-0 flex-col items-center border-r border-border bg-rail md:flex">
        <div className="flex w-full justify-center pt-3 pb-2">
          <RailButton
            active={allActive}
            label={t("settings.scenarios")}
            onClick={openAllScenarios}
          >
            <LayoutGrid className="h-5 w-5" />
          </RailButton>
        </div>
        <div className="no-scrollbar flex min-h-0 w-full flex-1 flex-col items-center gap-2 overflow-y-auto py-2">
          {scenes}
        </div>
        <div className="flex w-full justify-center py-3">
          <RailButton
            active={settingsActive}
            label={t("settings.title")}
            onClick={() => openSettings("preferences")}
          >
            <Settings className="h-5 w-5" />
          </RailButton>
        </div>
      </aside>
      <div className="no-scrollbar flex w-full shrink-0 items-center gap-2 overflow-x-auto border-b border-border bg-rail px-2 py-2 md:hidden">
        <RailButton
          active={allActive}
          label={t("settings.scenarios")}
          onClick={openAllScenarios}
        >
          <LayoutGrid className="h-5 w-5" />
        </RailButton>
        {scenes}
        <RailButton
          active={settingsActive}
          label={t("settings.title")}
          onClick={() => openSettings("preferences")}
        >
          <Settings className="h-5 w-5" />
        </RailButton>
      </div>

      <NewScenarioDialog
        open={creating}
        onOpenChange={setCreating}
        onCreated={() => navigate(SCENARIO_HOME)}
      />
    </>
  );
}
