import { useMutation } from "@tanstack/react-query";
import { useState } from "react";
import {
  Dialog,
  DialogTrigger,
  Form,
  Heading,
  Label,
  Modal,
  ModalOverlay,
  Radio,
  RadioGroup,
} from "react-aria-components";
import { keys, queryClient } from "../../app/query";
import { errorMessage } from "../../i18n/errors";
import { api, type Member } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { TextField } from "../../ui/TextField";
import { MEMBER_COLORS } from "./palette";

/** Limites du cœur, alignées sur PluralKit. */
const MAX_NAME = 100;
const MAX_PRONOUNS = 100;

const PALETTE: readonly string[] = MEMBER_COLORS;
const byName = (a: Member, b: Member) => a.name.localeCompare(b.name);

/** Création si `member` est absent, modification sinon. */
export function MemberDialog({ member }: { member?: Member }) {
  return (
    <ModalOverlay className="overlay" isDismissable>
      <Modal className="modal">
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

  const save = useMutation({
    mutationFn: (input: { name: string; pronouns: string | null; color: string }) =>
      member ? api.members.update(member.id, input) : api.members.create(input),
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
