// Create a scenario: name + surface (web console or one-shot task). The
// scenario store selects the new scenario on success; `onCreated` only has
// to fix the route.

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Globe, TerminalSquare } from "lucide-react";
import { useScenarioStore } from "@/app/store/scenarios";
import { useBlocking } from "@/app/store/busy";
import type { Surface } from "@/shared/api/api";
import { cn } from "@/shared/lib/utils";
import { Button } from "@/shared/ui/button";
import { Input } from "@/shared/ui/input";
import { Label } from "@/shared/ui/label";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/shared/ui/dialog";

export function NewScenarioDialog({
  open,
  onOpenChange,
  onCreated,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onCreated: (id: string) => void;
}) {
  const { t } = useTranslation();
  const createScenario = useScenarioStore((s) => s.createScenario);
  const busy = useBlocking();
  const [name, setName] = useState("");
  const [surface, setSurface] = useState<Surface>("web");

  const close = () => {
    onOpenChange(false);
    setName("");
    setSurface("web");
  };

  const create = async () => {
    const trimmed = name.trim();
    if (!trimmed || busy) return;
    try {
      await createScenario(trimmed, surface);
    } catch {
      return;
    }
    close();
    onCreated(trimmed);
  };

  return (
    <Dialog open={open} onOpenChange={(next) => !next && close()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("settings.newScenarioTitle")}</DialogTitle>
        </DialogHeader>
        <SurfacePicker value={surface} onChange={setSurface} />
        <Input
          value={name}
          onChange={(event) => setName(event.target.value)}
          placeholder={t("settings.profileName")}
          autoFocus
          onKeyDown={(event) => {
            if (event.nativeEvent.isComposing) return;
            if (event.key === "Enter") create();
          }}
        />
        <DialogFooter>
          <Button variant="secondary" onClick={close}>
            {t("settings.cancel")}
          </Button>
          <Button variant="primary" disabled={!name.trim() || busy} onClick={create}>
            {t("settings.addProfile")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export function SurfacePicker({
  value,
  onChange,
}: {
  value: Surface;
  onChange: (surface: Surface) => void;
}) {
  const { t } = useTranslation();
  const options: { id: Surface; icon: typeof Globe; title: string; hint: string }[] = [
    {
      id: "web",
      icon: Globe,
      title: t("settings.surfaceWeb"),
      hint: t("settings.surfaceWebHint"),
    },
    {
      id: "task",
      icon: TerminalSquare,
      title: t("settings.surfaceTask"),
      hint: t("settings.surfaceTaskHint"),
    },
  ];

  return (
    <div className="space-y-2">
      <Label>{t("settings.surfaceLabel")}</Label>
      <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
        {options.map((option) => {
          const Icon = option.icon;
          const active = value === option.id;
          return (
            <button
              key={option.id}
              type="button"
              onClick={() => onChange(option.id)}
              className={cn(
                "rounded-md border p-3 text-left transition-colors",
                active
                  ? "border-accent/40 bg-accent/10"
                  : "border-border bg-panel-2 hover:bg-hover",
              )}
            >
              <div className="flex items-center gap-2 text-body font-semibold text-text">
                <Icon className="h-4 w-4 text-accent" />
                {option.title}
              </div>
              <div className="mt-1 text-small text-text-dim">{option.hint}</div>
            </button>
          );
        })}
      </div>
    </div>
  );
}
