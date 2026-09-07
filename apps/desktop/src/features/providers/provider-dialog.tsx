// Creating a provider only needs an id and a display name; the connection
// fields (Base URL / API format / key) are edited inline on the detail side
// right after creation.

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useProviderStore } from "@/app/store/providers";
import { Button } from "@/shared/ui/button";
import { Input } from "@/shared/ui/input";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/shared/ui/dialog";
import { Field } from "./field";

export function NewProviderDialog({
  profile,
  open,
  onClose,
  onCreated,
}: {
  profile: string;
  open: boolean;
  onClose: () => void;
  onCreated: (id: string) => void;
}) {
  const { t } = useTranslation();
  const upsertProvider = useProviderStore((s) => s.upsertProvider);

  const [id, setId] = useState("");
  const [name, setName] = useState("");

  const canSave = id.trim().length > 0 && name.trim().length > 0;

  const save = async () => {
    if (!canSave) return;
    await upsertProvider(profile, {
      id: id.trim(),
      name: name.trim(),
      kind: "pi-ai",
      api: null,
      base_url: null,
      api_key_env: null,
      compat: null,
    });
    onCreated(id.trim());
    setId("");
    setName("");
    onClose();
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) {
          setId("");
          setName("");
          onClose();
        }
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("settings.addProvider")}</DialogTitle>
        </DialogHeader>
        <div className="space-y-3">
          <Field label={t("settings.id")}>
            <Input
              value={id}
              onChange={(event) => setId(event.target.value)}
              placeholder="openai"
              spellCheck={false}
              autoFocus
            />
          </Field>
          <Field label={t("settings.name")}>
            <Input
              value={name}
              onChange={(event) => setName(event.target.value)}
              placeholder="OpenAI"
            />
          </Field>
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>
            {t("settings.cancel")}
          </Button>
          <Button variant="primary" onClick={save} disabled={!canSave}>
            {t("settings.save")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
