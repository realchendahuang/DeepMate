// Creating a provider needs an id and a display name; the connection
// fields (Base URL / API format / key) are edited inline on the detail side
// right after creation. The advanced compat block is optional JSON and is
// edited here on creation so a new provider never loses it.

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
import { Textarea } from "@/shared/ui/textarea";
import { Field } from "./field";
import { AdvancedSection } from "./json-field";
import { jsonOrNull, jsonValid } from "./json-utils";

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
  const [compat, setCompat] = useState("");

  const compatValid = jsonValid(compat);

  const canSave = id.trim().length > 0 && name.trim().length > 0 && compatValid;

  const save = async () => {
    if (!canSave) return;
    await upsertProvider(profile, {
      id: id.trim(),
      name: name.trim(),
      kind: "pi-ai",
      api: null,
      base_url: null,
      api_key_env: null,
      compat: jsonOrNull(compat),
    });
    onCreated(id.trim());
    setId("");
    setName("");
    setCompat("");
    onClose();
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) {
          setId("");
          setName("");
          setCompat("");
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
          <AdvancedSection>
            <Field label={t("settings.compat")}>
              <Textarea
                value={compat}
                onChange={(event) => setCompat(event.target.value)}
                placeholder={t("settings.jsonPlaceholder", { key: "supportsStore" })}
                spellCheck={false}
                className={compatValid ? "h-20" : "h-20 border-warn focus-visible:border-warn"}
              />
              <p className={compatValid ? "text-caption text-text-faint" : "text-caption text-warn"}>
                {compatValid ? t("settings.compatHint") : t("settings.invalidJson")}
              </p>
            </Field>
          </AdvancedSection>
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