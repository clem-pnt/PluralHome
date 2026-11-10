import { useMutation } from "@tanstack/react-query";
import { useState } from "react";
import {
  Dialog,
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

export function NewMemberDialog() {
  return (
    <ModalOverlay className="overlay" isDismissable>
      <Modal className="modal">
        <Dialog className="dialog">{({ close }) => <NewMemberForm onDone={close} />}</Dialog>
      </Modal>
    </ModalOverlay>
  );
}

function NewMemberForm({ onDone }: { onDone: () => void }) {
  const [name, setName] = useState("");
  const [pronouns, setPronouns] = useState("");
  const [color, setColor] = useState<string>(MEMBER_COLORS[0]);

  const create = useMutation({
    mutationFn: api.members.create,
    onSuccess: (member) => {
      queryClient.setQueryData<Member[]>(keys.members, (prev = []) =>
        [...prev, member].sort((a, b) => a.name.localeCompare(b.name)),
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
        create.mutate({ name, pronouns: pronouns || null, color });
      }}
    >
      <Heading slot="title">{m.member_form_title()}</Heading>
      <TextField
        label={m.member_name()}
        autoFocus
        isRequired
        maxLength={64}
        value={name}
        onChange={setName}
      />
      <TextField
        label={m.member_pronouns()}
        hint={m.member_pronouns_hint()}
        maxLength={32}
        value={pronouns}
        onChange={setPronouns}
      />
      <RadioGroup className="field" value={color} onChange={setColor} orientation="horizontal">
        <Label className="field__label">{m.member_color()}</Label>
        <div className="swatches">
          {MEMBER_COLORS.map((c) => (
            <Radio key={c} value={c} aria-label={c} className="swatch" style={{ background: c }} />
          ))}
        </div>
      </RadioGroup>
      {create.isError && (
        <p className="alert" role="alert">
          {errorMessage(create.error)}
        </p>
      )}
      <div className="dialog__actions">
        <Button variant="ghost" slot="close">
          {m.cancel()}
        </Button>
        <Button type="submit" isDisabled={create.isPending}>
          {m.member_save()}
        </Button>
      </div>
    </Form>
  );
}
