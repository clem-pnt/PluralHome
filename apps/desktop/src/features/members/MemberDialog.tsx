import { useMutation } from "@tanstack/react-query";
import { useState } from "react";
import {
  TextField as AriaTextField,
  Dialog,
  DialogTrigger,
  FileTrigger,
  Form,
  Heading,
  Label,
  Modal,
  ModalOverlay,
  Radio,
  RadioGroup,
  TextArea,
} from "react-aria-components";
import { keys, queryClient } from "../../app/query";
import { errorMessage } from "../../i18n/errors";
import { type AvatarEdit, api, type Member } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Avatar } from "../../ui/Avatar";
import { Button } from "../../ui/Button";
import { TextField } from "../../ui/TextField";
import { squareAvatar } from "./image";
import { MEMBER_COLORS } from "./palette";
import { RolesField } from "./RolesField";

/** Limites du cœur, alignées sur PluralKit. */
const MAX_NAME = 100;
const MAX_PRONOUNS = 100;
const MAX_DESCRIPTION = 2000;

const PALETTE: readonly string[] = MEMBER_COLORS;
const byName = (a: Member, b: Member) => a.name.localeCompare(b.name);

/** Création si `member` est absent, modification sinon. */
export function MemberDialog({ member }: { member?: Member }) {
  return (
    <ModalOverlay className="overlay" isDismissable>
      <Modal className="modal modal--wide">
        <Dialog className="dialog">
          {({ close }) => <MemberForm member={member} onDone={close} />}
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}

function MemberForm({ member, onDone }: { member: Member | undefined; onDone: () => void }) {
  const [name, setName] = useState(member?.name ?? "");
  const [pronouns, setPronouns] = useState(member?.pronouns ?? "");
  const [color, setColor] = useState<string>(member?.color ?? MEMBER_COLORS[0]);
  // Une couleur venue de PluralKit peut ne pas être dans la palette : on la propose aussi.
  const original = member?.color;
  const colors = original && !PALETTE.includes(original) ? [...PALETTE, original] : PALETTE;

  const [description, setDescription] = useState(member?.description ?? "");
  const [roles, setRoles] = useState<string[]>(member?.roles ?? []);
  // Aperçu affiché (adresse data:) et modification à envoyer au cœur.
  const [avatar, setAvatar] = useState<string | null>(member?.avatar ?? null);
  const [avatarEdit, setAvatarEdit] = useState<AvatarEdit>({ kind: "keep" });
  const [avatarError, setAvatarError] = useState(false);

  const pickAvatar = async (files: FileList | null) => {
    const file = files?.[0];
    if (!file) return;
    try {
      const { data, preview } = await squareAvatar(file);
      setAvatarError(false);
      setAvatarEdit({ kind: "set", data });
      setAvatar(preview);
    } catch {
      setAvatarError(true);
    }
  };

  const save = useMutation({
    mutationFn: async (input: { name: string; pronouns: string | null; color: string }) => {
      const saved = member
        ? await api.members.update(member.id, input)
        : await api.members.create(input);
      return api.members.setProfile(saved.id, {
        description: description.trim() || null,
        roles,
        avatar: avatarEdit,
      });
    },
    onSuccess: (saved) => {
      queryClient.setQueryData<Member[]>(keys.members, (prev = []) =>
        [...prev.filter((x) => x.id !== saved.id), saved].sort(byName),
      );
      onDone();
    },
  });

  return (
    <Form
      className="dialog"
      style={{ padding: 0 }}
      onSubmit={(e) => {
        e.preventDefault();
        save.mutate({ name, pronouns: pronouns.trim() || null, color });
      }}
    >
      <Heading slot="title">
        {member ? m.member_form_title_edit({ name: member.name }) : m.member_form_title()}
      </Heading>
      <div className="avatar-edit">
        <Avatar name={name || "?"} color={color} src={avatar} size={72} />
        <div className="avatar-edit__actions">
          <FileTrigger
            acceptedFileTypes={["image/png", "image/jpeg", "image/webp"]}
            onSelect={(files) => void pickAvatar(files)}
          >
            <Button variant="ghost">
              {avatar ? m.member_avatar_change() : m.member_avatar_add()}
            </Button>
          </FileTrigger>
          {avatar && (
            <Button
              variant="ghost"
              onPress={() => {
                setAvatar(null);
                setAvatarEdit({ kind: "remove" });
              }}
            >
              {m.member_avatar_remove()}
            </Button>
          )}
        </div>
      </div>
      {avatarError && (
        <p className="alert" role="alert">
          {m.member_avatar_error()}
        </p>
      )}
      <TextField
        label={m.member_name()}
        autoFocus
        isRequired
        maxLength={MAX_NAME}
        value={name}
        onChange={setName}
      />
      <TextField
        label={m.member_pronouns()}
        hint={m.member_pronouns_hint()}
        maxLength={MAX_PRONOUNS}
        value={pronouns}
        onChange={setPronouns}
      />
      <RadioGroup className="field" value={color} onChange={setColor} orientation="horizontal">
        <Label className="field__label">{m.member_color()}</Label>
        <div className="swatches">
          {colors.map((c) => (
            <Radio key={c} value={c} aria-label={c} className="swatch" style={{ background: c }} />
          ))}
        </div>
      </RadioGroup>
      <AriaTextField
        className="field"
        maxLength={MAX_DESCRIPTION}
        value={description}
        onChange={setDescription}
      >
        <Label className="field__label">{m.member_description()}</Label>
        <TextArea className="input input--area" rows={4} />
        <span className="field__hint">{m.member_description_hint()}</span>
      </AriaTextField>
      <RolesField roles={roles} onChange={setRoles} />
      <p className="field__hint">{m.member_profile_local()}</p>
      {save.isError && (
        <p className="alert" role="alert">
          {errorMessage(save.error)}
        </p>
      )}
      <div className="dialog__actions">
        {member && (
          <span className="dialog__aside">
            <ArchiveButton member={member} onDone={onDone} />
          </span>
        )}
        <Button variant="ghost" slot="close">
          {m.cancel()}
        </Button>
        <Button type="submit" isDisabled={save.isPending}>
          {member ? m.member_update() : m.member_save()}
        </Button>
      </div>
    </Form>
  );
}

function ArchiveButton({ member, onDone }: { member: Member; onDone: () => void }) {
  const archive = useMutation({
    mutationFn: () => api.members.archive(member.id),
    onSuccess: () => {
      queryClient.setQueryData<Member[]>(keys.members, (prev = []) =>
        prev.filter((x) => x.id !== member.id),
      );
      // L'archivage ferme aussi un éventuel passage au front.
      void queryClient.invalidateQueries({ queryKey: ["front"] });
      onDone();
    },
  });

  return (
    <DialogTrigger>
      <Button variant="ghost">{m.member_archive()}</Button>
      <ModalOverlay className="overlay" isDismissable>
        <Modal className="modal">
          <Dialog className="dialog" role="alertdialog">
            {({ close }) => (
              <>
                <Heading slot="title">{m.member_archive_title({ name: member.name })}</Heading>
                <p className="muted">{m.member_archive_body({ name: member.name })}</p>
                {archive.isError && (
                  <p className="alert" role="alert">
                    {errorMessage(archive.error)}
                  </p>
                )}
                <div className="dialog__actions">
                  <Button variant="ghost" onPress={close}>
                    {m.cancel()}
                  </Button>
                  <Button
                    variant="danger"
                    autoFocus
                    isDisabled={archive.isPending}
                    onPress={() => archive.mutate()}
                  >
                    {m.member_archive()}
                  </Button>
                </div>
              </>
            )}
          </Dialog>
        </Modal>
      </ModalOverlay>
    </DialogTrigger>
  );
}
