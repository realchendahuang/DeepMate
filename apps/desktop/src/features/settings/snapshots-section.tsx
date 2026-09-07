// Snapshots: export the current state under a name, import or delete
// existing ones.

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Layers, Plus, Trash2 } from "lucide-react";
import { useSnapshotStore } from "@/app/store/snapshots";
import { useBusyStore } from "@/app/store/busy";
import { Card, CardContent } from "@/shared/ui/card";
import { Button } from "@/shared/ui/button";
import { Input } from "@/shared/ui/input";
import { Skeleton } from "@/shared/ui/skeleton";
import { EmptyState } from "@/shared/ui/empty-state";
import { ConfirmDialog } from "@/shared/ui/confirm-dialog";

export function SnapshotsSection() {
  const { t } = useTranslation();
  const snapshots = useSnapshotStore((s) => s.snapshots);
  const snapshotsLoaded = useSnapshotStore((s) => s.snapshotsLoaded);
  const loadSnapshots = useSnapshotStore((s) => s.loadSnapshots);
  const snapshotExport = useSnapshotStore((s) => s.snapshotExport);
  const snapshotImport = useSnapshotStore((s) => s.snapshotImport);
  const snapshotDelete = useSnapshotStore((s) => s.snapshotDelete);
  const busy = useBusyStore((s) => s.busyAction) !== null;

  const [snapshotName, setSnapshotName] = useState("");
  // Which snapshot/import is pending confirmation.
  const [confirm, setConfirm] = useState<{ kind: "delete" | "import"; name: string } | null>(
    null,
  );

  useEffect(() => {
    void loadSnapshots();
  }, [loadSnapshots]);

  const runExport = () => {
    if (!snapshotName.trim()) return;
    snapshotExport(snapshotName.trim());
    setSnapshotName("");
  };

  const runConfirm = () => {
    if (!confirm) return;
    const { kind, name } = confirm;
    setConfirm(null);
    if (kind === "delete") snapshotDelete(name);
    else snapshotImport(name);
  };

  return (
    <section className="space-y-3">
      <Card>
        <CardContent className="flex flex-col gap-2 p-4 md:flex-row">
          <Input
            value={snapshotName}
            onChange={(event) => setSnapshotName(event.target.value)}
            placeholder={t("settings.snapshotName")}
            className="flex-1"
            onKeyDown={(event) => {
              if (event.key === "Enter" && snapshotName.trim()) runExport();
            }}
          />
          <Button variant="primary" onClick={runExport} disabled={!snapshotName.trim() || busy}>
            <Plus className="h-4 w-4" />
            {t("settings.exportSnapshot")}
          </Button>
        </CardContent>
      </Card>

      {!snapshotsLoaded ? (
        <Skeleton className="h-24 w-full" />
      ) : snapshots.length === 0 ? (
        <EmptyState icon={<Layers className="h-8 w-8 text-text-faint" />}>
          {t("settings.noSnapshots")}
        </EmptyState>
      ) : (
        <Card className="overflow-hidden">
          <div className="divide-y divide-border">
            {snapshots.map((name) => (
              <div
                key={name}
                className="flex items-center gap-3 px-4 py-2.5 transition-colors hover:bg-hover md:px-5"
              >
                <div className="min-w-0 flex-1 truncate text-body font-semibold text-text">
                  {name}
                </div>
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() => setConfirm({ kind: "import", name })}
                  disabled={busy}
                >
                  {t("settings.importSnapshot")}
                </Button>
                <Button
                  variant="danger"
                  size="sm"
                  onClick={() => setConfirm({ kind: "delete", name })}
                  disabled={busy}
                >
                  <Trash2 className="h-4 w-4" />
                  {t("settings.deleteSnapshot")}
                </Button>
              </div>
            ))}
          </div>
        </Card>
      )}

      <ConfirmDialog
        open={confirm !== null}
        onOpenChange={(open) => !open && setConfirm(null)}
        title={
          confirm?.kind === "import"
            ? t("settings.importSnapshotConfirmTitle")
            : t("settings.deleteSnapshotConfirmTitle")
        }
        body={
          confirm?.kind === "import"
            ? t("settings.importSnapshotConfirmBody", { name: confirm.name })
            : t("settings.deleteSnapshotConfirmBody", { name: confirm?.name ?? "" })
        }
        onConfirm={runConfirm}
      />
    </section>
  );
}
