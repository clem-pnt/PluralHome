import { useState } from "react";
import { FileTrigger } from "react-aria-components";
import type { AvatarEdit } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Avatar } from "../../ui/Avatar";
import { Button } from "../../ui/Button";
import { squareAvatar } from "./image";

/** Choix d'une photo de profil : aperçu, changer, retirer. */
export function AvatarInput({
  name,
  preview,
  onChange,
  size = 72,
}: {
  name: string;
  preview: string | null;
  onChange: (edit: AvatarEdit, preview: string | null) => void;
  size?: number;
}) {
  const [error, setError] = useState(false);
  const pick = async (files: FileList | null) => {
    const file = files?.[0];
    if (!file) return;
    try {
      const { data, preview } = await squareAvatar(file);
      setError(false);
      onChange({ kind: "set", data }, preview);
    } catch {
      setError(true);
    }
  };
  return (
    <>
      <div className="avatar-edit">
        <Avatar name={name || "?"} src={preview} size={size} />
        <div className="avatar-edit__actions">
          <FileTrigger
            acceptedFileTypes={["image/png", "image/jpeg", "image/webp"]}
            onSelect={(files) => void pick(files)}
          >
            <Button variant="ghost">
              {preview ? m.member_avatar_change() : m.member_avatar_add()}
            </Button>
          </FileTrigger>
          {preview && (
            <Button variant="ghost" onPress={() => onChange({ kind: "remove" }, null)}>
              {m.member_avatar_remove()}
            </Button>
          )}
        </div>
      </div>
      {error && (
        <p className="alert" role="alert">
          {m.member_avatar_error()}
        </p>
      )}
    </>
  );
}
