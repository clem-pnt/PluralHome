import { useMutation } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { Form, Input, Label, Radio, RadioGroup, TextField } from "react-aria-components";
import { keys, queryClient } from "../../app/query";
import { errorMessage } from "../../i18n/errors";
import { type AvatarEdit, api, type SystemKind } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { AvatarInput } from "../members/AvatarInput";
import { useSystem } from "../messages/useGroups";

/**
 * Nom et photo du système, repris de PluralKit à l'import ou choisis ici.
 * Ils donnent son visage à la conversation de tout le système.
 */
export function SystemPanel() {
  const system = useSystem();
  const [name, setName] = useState("");
  const [preview, setPreview] = useState<string | null>(null);
  const [avatar, setAvatar] = useState<AvatarEdit>({ kind: "keep" });
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    if (!system.data) return;
    setName(system.data.name ?? "");
    setPreview(system.data.avatar ?? null);
    setAvatar({ kind: "keep" });
  }, [system.data]);

  const save = useMutation({
    mutationFn: () => api.system.set({ name: name.trim() || null, avatar }),
    onSuccess: (profile) => {
      queryClient.setQueryData(keys.system, profile);
      setSaved(true);
    },
  });

  const setKind = useMutation({
    mutationFn: api.system.setKind,
    onSuccess: (profile) => queryClient.setQueryData(keys.system, profile),
  });

  return (
    <>
      <RadioGroup
        className="field"
        value={system.data?.kind ?? "system"}
        isDisabled={!system.data || setKind.isPending}
        onChange={(v) => setKind.mutate(v as SystemKind)}
        orientation="horizontal"
      >
        <Label className="field__label">{m.system_kind()}</Label>
        <div className="choices">
          <Radio value="system" className="choice">
            {m.system_kind_system()}
          </Radio>
          <Radio value="singlet" className="choice">
            {m.system_kind_singlet()}
          </Radio>
        </div>
        <p className="muted">{m.system_kind_hint()}</p>
        {setKind.isError && (
          <p className="alert" role="alert">
            {errorMessage(setKind.error)}
          </p>
        )}
      </RadioGroup>
      <Form
        className="system-panel"
        onSubmit={(e) => {
          e.preventDefault();
          save.mutate();
        }}
      >
        <AvatarInput
          name={name || m.messages_everyone()}
          preview={preview}
          onChange={(edit, next) => {
            setAvatar(edit);
            setPreview(next);
            setSaved(false);
          }}
        />
        <TextField
          className="field"
          value={name}
          onChange={(v) => {
            setName(v);
            setSaved(false);
          }}
          maxLength={100}
        >
          <Label>{m.system_name()}</Label>
          <Input className="input" placeholder={m.messages_everyone()} />
        </TextField>
        {save.isError && (
          <p className="alert" role="alert">
            {errorMessage(save.error)}
          </p>
        )}
        <div className="dialog__actions">
          {saved && (
            <span className="muted" role="status">
              {m.system_saved()}
            </span>
          )}
          <Button variant="primary" type="submit" isDisabled={save.isPending || system.isPending}>
            {m.messages_save()}
          </Button>
        </div>
      </Form>
    </>
  );
}
