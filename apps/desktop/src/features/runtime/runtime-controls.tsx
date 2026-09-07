// Start / open / restart / stop for a web-surface scenario, shared by the
// scenario overview and the all-scenarios inventory.

import { useTranslation } from "react-i18next";
import { ExternalLink, Play, RotateCw, Square } from "lucide-react";
import { useRuntimeStore } from "@/app/store/runtime";
import { useBusyStore } from "@/app/store/busy";
import { Button } from "@/shared/ui/button";

export function WebRuntimeControls({
  profileId,
  running,
  pid,
  size = "default",
}: {
  profileId: string;
  running: boolean;
  pid?: number | null;
  size?: "default" | "sm";
}) {
  const { t } = useTranslation();
  const runtimeStart = useRuntimeStore((s) => s.runtimeStart);
  const runtimeStop = useRuntimeStore((s) => s.runtimeStop);
  const runtimeRestart = useRuntimeStore((s) => s.runtimeRestart);
  const openHarness = useRuntimeStore((s) => s.openHarness);
  const busy = useBusyStore((s) => s.busyAction) !== null;

  if (!running) {
    return (
      <Button
        variant="primary"
        size={size}
        onClick={() => runtimeStart(profileId)}
        disabled={busy}
      >
        <Play className="h-4 w-4" />
        {t("overview.start")}
      </Button>
    );
  }

  return (
    <>
      <Button
        variant="primary"
        size={size}
        onClick={() => openHarness(profileId)}
        disabled={busy}
      >
        <ExternalLink className="h-4 w-4" />
        {t("overview.openHarness")}
      </Button>
      <Button
        size={size}
        onClick={() => runtimeRestart(profileId)}
        disabled={busy || pid == null}
      >
        <RotateCw className="h-4 w-4" />
        {t("overview.restart")}
      </Button>
      <Button
        variant="danger"
        size={size}
        onClick={() => runtimeStop(profileId)}
        disabled={busy || pid == null}
      >
        <Square className="h-4 w-4" />
        {t("overview.stop")}
      </Button>
    </>
  );
}
