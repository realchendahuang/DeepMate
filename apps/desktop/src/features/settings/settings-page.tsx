// System-wide settings behind the rail's gear. Scene-owned config
// (providers, models, plugins, identity) lives in the scenario sections.

import { useTranslation } from "react-i18next";
import type { SettingSection } from "@/app/router";
import { PageBody, PageHeader } from "@/shared/ui/page";
import { DiagnosticsPage } from "@/features/diagnostics/diagnostics-page";
import { PreferencesSection } from "./preferences-section";
import { SnapshotsSection } from "./snapshots-section";
import { AboutSection } from "./about-section";

const SECTION_TITLE: Record<SettingSection, string> = {
  diagnostics: "settings.diagnostics",
  snapshots: "settings.snapshots",
  preferences: "settings.preferences",
  about: "settings.about",
};

export function SettingsPage({ section }: { section: SettingSection }) {
  const { t } = useTranslation();

  if (section === "diagnostics") {
    return <DiagnosticsPage />;
  }

  return (
    <PageBody className="space-y-5">
      <PageHeader title={t(SECTION_TITLE[section])} />
      {section === "preferences" && <PreferencesSection />}
      {section === "snapshots" && <SnapshotsSection />}
      {section === "about" && <AboutSection />}
    </PageBody>
  );
}
