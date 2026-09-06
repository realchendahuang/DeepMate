// Surface identity chip: web scenarios open a browser console, task scenarios
// answer one-shot task runs.
import { useTranslation } from "react-i18next";
import { Globe, TerminalSquare } from "lucide-react";
import { Badge } from "../ui/badge";
import type { Surface } from "../../api";

export function SurfaceBadge({ surface }: { surface: Surface }) {
  const { t } = useTranslation();
  if (surface === "web") {
    return (
      <Badge variant="accent" dot={false}>
        <Globe className="h-3 w-3" />
        {t("settings.surfaceWeb")}
      </Badge>
    );
  }
  if (surface === "task") {
    return (
      <Badge variant="neutral" dot={false}>
        <TerminalSquare className="h-3 w-3" />
        {t("settings.surfaceTask")}
      </Badge>
    );
  }
  return (
    <Badge variant="skip" dot={false}>
      {t("settings.surfaceUndetermined")}
    </Badge>
  );
}
