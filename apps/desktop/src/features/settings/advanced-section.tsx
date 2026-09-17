// Advanced: raw edit of the harness's own configuration documents. Only
// these known files are reachable; everything else stays in the structured
// editors. settings.yaml saves are validated as YAML on the backend;
// cordis.patch.yml is written verbatim (its `dshHomePath` custom tag cannot
// be parsed as plain YAML), so the warning under it is a real constraint.

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { FileCode2, FileText, SlidersHorizontal } from "lucide-react";
import { toast } from "sonner";
import { api } from "@/shared/api/api";
import { DEFAULT_SCENARIO_ID } from "@/shared/lib/scenario";
import i18n from "@/i18n";
import { mapError } from "@/shared/lib/errors";
import { useScenarioStore } from "@/app/store/scenarios";
import { runBusy, useBlocking } from "@/app/store/busy";
import { Card } from "@/shared/ui/card";
import { Button } from "@/shared/ui/button";
import { Badge } from "@/shared/ui/badge";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/shared/ui/dialog";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/shared/ui/select";
import { Skeleton } from "@/shared/ui/skeleton";
import { Textarea } from "@/shared/ui/textarea";

interface AdvancedFile {
  scope: string;
  name: string | null;
  // Stable dialog key so the draft reseeds per file.
  key: string;
  titleKey: string;
}

export function AdvancedSection() {
  const { t } = useTranslation();
  const profiles = useScenarioStore((s) => s.profiles);
  const selectedScenario = useScenarioStore((s) => s.selectedScenario);
  const busy = useBlocking();

  // The scenario the scene files are scoped to. It follows the focused
  // scenario by default and a manual pick in the dropdown takes over. The
  // follow is derived rather than synced in an effect: `picked` records only
  // what the user chose, so a scenario switch outside this page cannot leave
  // a stale selection behind.
  const [picked, setPicked] = useState<string | null>(null);
  const scene = picked ?? selectedScenario ?? DEFAULT_SCENARIO_ID;
  const [file, setFile] = useState<AdvancedFile | null>(null);

  const sceneFiles: AdvancedFile[] = [
    {
      scope: "scene-settings",
      name: scene,
      key: `scene-settings-${scene}`,
      titleKey: "settings.advancedSceneSettings",
    },
    {
      scope: "scene-cordis",
      name: scene,
      key: `scene-cordis-${scene}`,
      titleKey: "settings.advancedSceneCordis",
    },
  ];

  return (
    <section className="space-y-3">
      <Card>
        <div className="divide-y divide-border">
          <button
            type="button"
            className="flex w-full items-center gap-3 p-4 text-left transition-colors hover:bg-hover"
            onClick={() =>
              setFile({
                scope: "global-settings",
                name: null,
                key: "global-settings",
                titleKey: "settings.advancedGlobalSettings",
              })
            }
          >
            <FileText className="h-4 w-4 shrink-0 text-text-dim" />
            <span className="min-w-0 flex-1">
              <span className="block text-heading font-semibold text-text">
                {t("settings.advancedGlobalSettings")}
              </span>
              <span className="block truncate font-mono text-caption text-text-faint">
                settings.yaml
              </span>
            </span>
            <span className="shrink-0">
              <Badge variant="neutral" dot={false}>
                {t("settings.advancedEdit")}
              </Badge>
            </span>
          </button>

          <div className="space-y-3 p-4">
            <div className="flex items-center gap-2">
              <SlidersHorizontal className="h-4 w-4 text-text-dim" />
              <span className="text-heading font-semibold text-text">
                {t("settings.advancedSceneFiles")}
              </span>
              <Select
                value={scene}
                onValueChange={(value) => setPicked(value)}
                disabled={busy}
                aria-label={t("settings.advancedSceneFiles")}
              >
                <SelectTrigger className="ml-auto w-auto min-w-0 max-w-[220px]">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {profiles.map((profile) => (
                    <SelectItem key={profile.id} value={profile.id}>
                      {profile.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            {sceneFiles.map((item) => (
              <button
                key={item.key}
                type="button"
                className="flex w-full items-center gap-3 rounded-md border border-border p-3 text-left transition-colors hover:bg-hover"
                onClick={() => setFile(item)}
              >
                <FileCode2 className="h-4 w-4 shrink-0 text-text-dim" />
                <span className="min-w-0 flex-1">
                  <span className="block text-body font-semibold text-text">
                    {t(item.titleKey)}
                  </span>
                  <span className="block truncate font-mono text-caption text-text-faint">
                    profiles/{scene}/
                    {item.scope === "scene-cordis" ? "cordis.patch.yml" : "settings.yaml"}
                  </span>
                </span>
                <span className="shrink-0">
                  <Badge variant="neutral" dot={false}>
                    {item.scope === "scene-cordis" ? t("settings.advancedCordisBadge") : "YAML"}
                  </Badge>
                </span>
              </button>
            ))}
            <p className="text-caption text-text-faint">{t("settings.advancedHint")}</p>
          </div>
        </div>
      </Card>

      {/* Mounted per file: the key reseeds the draft for each document. */}
      {file && <AdvancedFileDialog key={file.key} file={file} onClose={() => setFile(null)} />}
    </section>
  );
}

// One raw file in a dialog: load on mount, validate on the backend on save
// (settings.yaml) or write verbatim (cordis.patch.yml), with a backup made
// before each write. Mounted only while a file is selected, and keyed per
// file so the draft always reseeds.
function AdvancedFileDialog({ file, onClose }: { file: AdvancedFile; onClose: () => void }) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState<string | null>(null);
  const [loadFailed, setLoadFailed] = useState(false);

  useEffect(() => {
    let alive = true;
    // The component is keyed per file, so the draft starts null; only the
    // async read result is set here.
    void api
      .advancedFileRead(file.scope, file.name)
      .then((content) => {
        if (alive) setDraft(content ?? "");
      })
      .catch((error: unknown) => {
        if (alive) {
          setLoadFailed(true);
          // i18n is consulted through its instance rather than the `t`
          // binding: including `t` in the dependency list would re-run this
          // read on every language change and discard the user's draft.
          toast.error(i18n.t(`common.errors.${mapError(error)}`));
        }
      });
    return () => {
      alive = false;
    };
  }, [file]);

  const save = async () => {
    if (draft === null) return;
    try {
      // Going through runBusy keeps this dialog consistent with every other
      // write in the app: controls disable while it runs, failures are
      // translated through the shared error mapping.
      await runBusy("save", () => api.advancedFileSave(file.scope, file.name, draft));
      toast.success(t("settings.advancedSaved"));
      onClose();
    } catch {
      // runBusy already surfaced it.
    }
  };

  const saved = draft !== null;

  return (
    <Dialog open onOpenChange={(next) => !next && onClose()}>
      <DialogContent className="sm:max-w-[560px]">
        <DialogHeader>
          <DialogTitle>{t(file.titleKey)}</DialogTitle>
        </DialogHeader>
        {saved ? (
          <Textarea
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            spellCheck={false}
            rows={14}
            autoFocus
          />
        ) : loadFailed ? (
          <div className="text-body text-text-dim">{t("settings.advancedLoadFailed")}</div>
        ) : (
          <Skeleton className="h-56 w-full" />
        )}
        {file.scope === "scene-cordis" && saved && (
          <p className="text-caption text-text-faint">{t("settings.advancedCordisWarn")}</p>
        )}
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>
            {t("settings.cancel")}
          </Button>
          <Button variant="primary" onClick={save} disabled={!saved}>
            {t("settings.save")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
