import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Layers, Cloud, Cpu } from "lucide-react";
import { useStore } from "../store";
import { Card, CardContent } from "../components/ui/card";
import { Segmented } from "../components/ui/segmented";
import { DataList, DataListRow, EmptyState } from "../components/ui/data-list";
import { PageBody, PageHeader, SectionHeader } from "../components/ui/page";

export function SettingsPage() {
  const { t } = useTranslation();
  const [section, setSection] = useState(0); // 0 = configuration, 1 = preferences

  const profiles = useStore((s) => s.profiles);
  const providers = useStore((s) => s.providers);
  const models = useStore((s) => s.models);
  const loadProfiles = useStore((s) => s.loadProfiles);
  const loadProviders = useStore((s) => s.loadProviders);
  const loadModels = useStore((s) => s.loadModels);
  const setLanguage = useStore((s) => s.setLanguage);
  const setTheme = useStore((s) => s.setTheme);
  const language = useStore((s) => s.language);
  const theme = useStore((s) => s.theme);

  useEffect(() => {
    if (section === 0) {
      loadProfiles();
      loadProviders();
      loadModels();
    }
  }, [section, loadProfiles, loadProviders, loadModels]);

  return (
    <PageBody className="space-y-5">
      <PageHeader title={t("settings.title")} />

      <Card>
        <CardContent className="p-2">
          <Segmented
            variant="full"
            options={[t("settings.configuration"), t("settings.preferences")]}
            value={section}
            onChange={setSection}
          />
        </CardContent>
      </Card>

      {section === 0 && (
        <div className="space-y-5">
          <section className="space-y-3">
            <SectionHeader title={t("settings.profiles")} />
            {profiles.length === 0 ? (
              <EmptyState icon={<Layers className="h-8 w-8 text-text-faint" />}>
                {t("settings.noProfiles")}
              </EmptyState>
            ) : (
              <DataList
                columns={[
                  { label: t("settings.name") },
                  { label: "ID", className: "w-[180px]" },
                  { label: t("settings.description"), compact: true },
                ]}
              >
                {profiles.map((profile) => (
                  <DataListRow
                    key={profile.id}
                    primary={profile.name}
                    secondary={<span className="block max-w-[160px] truncate">{profile.id}</span>}
                    detail={profile.description ?? ""}
                  />
                ))}
              </DataList>
            )}
          </section>

          <section className="space-y-3">
            <SectionHeader title={t("settings.providers")} />
            {providers.length === 0 ? (
              <EmptyState icon={<Cloud className="h-8 w-8 text-text-faint" />}>
                {t("settings.noProviders")}
              </EmptyState>
            ) : (
              <DataList
                columns={[
                  { label: t("settings.name") },
                  { label: t("settings.kind"), className: "w-[160px]" },
                  { label: "ID", compact: true },
                ]}
              >
                {providers.map((provider) => (
                  <DataListRow
                    key={provider.id}
                    primary={provider.name}
                    secondary={<span className="block max-w-[140px] truncate">{provider.kind}</span>}
                    detail={provider.id}
                  />
                ))}
              </DataList>
            )}
          </section>

          <section className="space-y-3">
            <SectionHeader title={t("settings.models")} />
            {models.length === 0 ? (
              <EmptyState icon={<Cpu className="h-8 w-8 text-text-faint" />}>
                {t("settings.noModels")}
              </EmptyState>
            ) : (
              <DataList
                columns={[
                  { label: t("settings.name") },
                  { label: t("settings.provider"), className: "w-[180px]" },
                  { label: "ID", compact: true },
                ]}
              >
                {models.map((model) => (
                  <DataListRow
                    key={model.id}
                    primary={model.name}
                    secondary={<span className="block max-w-[160px] truncate">{model.provider ?? "-"}</span>}
                    detail={model.id}
                  />
                ))}
              </DataList>
            )}
          </section>
        </div>
      )}

      {section === 1 && (
        <section className="space-y-3">
          <SectionHeader title={t("settings.preferences")} />
          <Card>
            <div className="divide-y divide-border">
              <div className="flex flex-col gap-3 p-4 md:flex-row md:items-center md:justify-between">
                <div>
                  <div className="text-heading font-semibold text-text">{t("settings.language")}</div>
                  <div className="text-small text-text-dim">{t("settings.general")}</div>
                </div>
                <Segmented
                  options={["English", "简体中文"]}
                  value={language === "zh" ? 1 : 0}
                  onChange={(index) => setLanguage(index === 0 ? "en" : "zh")}
                />
              </div>
              <div className="flex flex-col gap-3 p-4 md:flex-row md:items-center md:justify-between">
                <div>
                  <div className="text-heading font-semibold text-text">{t("settings.theme")}</div>
                  <div className="text-small text-text-dim">{t("settings.appearance")}</div>
                </div>
                <Segmented
                  options={["System", "Light", "Dark"]}
                  value={Math.max(0, ["system", "light", "dark"].indexOf(theme))}
                  onChange={(index) => setTheme(["system", "light", "dark"][index])}
                />
              </div>
            </div>
          </Card>
        </section>
      )}
    </PageBody>
  );
}
