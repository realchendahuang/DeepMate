// About: the product mark, the app version and the detected harness info.

import { useTranslation } from "react-i18next";
import { appVersion } from "@/shared/api/bindings";
import { useRuntimeStore } from "@/app/store/runtime";
import { Card, CardContent } from "@/shared/ui/card";
import { BrandMark } from "@/app/shell/brand-mark";

export function AboutSection() {
  const { t } = useTranslation();
  const overview = useRuntimeStore((s) => s.overview);

  return (
    <section className="space-y-3">
      <Card>
        <CardContent className="flex items-center gap-4 p-5">
          <BrandMark className="h-12 w-12" />
          <div>
            <div className="text-title font-bold text-text">DeepMate</div>
            <div className="mt-1 text-small text-text-dim">
              {t("settings.aboutVersion", { version: appVersion })}
            </div>
          </div>
        </CardContent>
      </Card>
      {overview?.detection.harness && (
        <Card>
          <CardContent className="p-4">
            <div className="text-heading font-semibold text-text">
              {overview.detection.harness.name}
            </div>
            {overview.detection.harness.version && (
              <div className="mt-1 text-small text-text-dim">
                {t("settings.aboutVersion", { version: overview.detection.harness.version })}
              </div>
            )}
          </CardContent>
        </Card>
      )}
    </section>
  );
}
