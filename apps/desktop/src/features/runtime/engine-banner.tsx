// Engine-missing banner on the scenario overview: shown when the harness
// CLI itself was not detected, with the install command.

import { useTranslation } from "react-i18next";
import { useRuntimeStore } from "@/app/store/runtime";
import { Card, CardContent } from "@/shared/ui/card";

export function EngineMissingBanner() {
  const { t } = useTranslation();
  const overview = useRuntimeStore((s) => s.overview);
  if (overview === null || overview.detection.found) return null;

  return (
    <Card className="border-warn/40">
      <CardContent className="flex flex-col gap-2 p-4 md:flex-row md:items-center md:justify-between">
        <div className="min-w-0">
          <div className="text-heading font-semibold text-text">{t("overview.notDetected")}</div>
          <p className="mt-0.5 text-small text-text-dim">{t("overview.notDetectedHint")}</p>
        </div>
        <code className="shrink-0 rounded border border-border bg-panel-2 px-2 py-1 font-mono text-caption text-text-dim">
          {t("doctor.installCommand")}
        </code>
      </CardContent>
    </Card>
  );
}
