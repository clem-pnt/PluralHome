import { useState } from "react";
import {
  Checkbox,
  CheckboxGroup,
  Dialog,
  Heading,
  Modal,
  ModalOverlay,
} from "react-aria-components";
import { errorMessage } from "../../i18n/errors";
import type { FrontSession } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Avatar } from "../../ui/Avatar";
import { Button } from "../../ui/Button";
import { useMembers } from "../members/useMembers";
import { useFrontChange } from "./useFront";

const BLURRY = "__blurry__";

export function FrontDialog({ current }: { current: FrontSession[] }) {
  return (
    <ModalOverlay className="overlay" isDismissable>
      <Modal className="modal">
        <Dialog className="dialog">
          {({ close }) => <FrontForm current={current} onDone={close} />}
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}

function FrontForm({ current, onDone }: { current: FrontSession[]; onDone: () => void }) {
  const members = useMembers();
  const change = useFrontChange();
  const [selected, setSelected] = useState<string[]>(() =>
    current.map((s) => s.memberId ?? BLURRY),
  );

  const save = (values: string[]) =>
    change.mutate(
      { members: values.filter((v) => v !== BLURRY), blurry: values.includes(BLURRY) },
      { onSuccess: onDone },
    );

  return (
    <>
      <div>
        <Heading slot="title">{m.front_dialog_title()}</Heading>
        <p className="muted">{m.front_dialog_lead()}</p>
      </div>
      <CheckboxGroup
        className="picker"
        aria-label={m.front_dialog_title()}
        value={selected}
        onChange={setSelected}
      >
        {members.list.map((member) => (
          <Checkbox key={member.id} value={member.id} className="pick">
            <Avatar name={member.name} color={member.color} size={32} />
            <span className="pick__name">{member.name}</span>
          </Checkbox>
        ))}
        <Checkbox value={BLURRY} className="pick">
          <span
            className="avatar avatar--blurry"
            style={{ "--size": "32px" } as React.CSSProperties}
            aria-hidden
          />
          <span className="pick__name">
            {m.front_blurry()}
            <small className="muted">{m.front_blurry_hint()}</small>
          </span>
        </Checkbox>
      </CheckboxGroup>
      {change.isError && (
        <p className="alert" role="alert">
          {errorMessage(change.error)}
        </p>
      )}
      <div className="dialog__actions">
        <Button variant="ghost" onPress={() => save([])} isDisabled={change.isPending}>
          {m.front_clear()}
        </Button>
        <span style={{ flex: 1 }} />
        <Button variant="ghost" slot="close">
          {m.cancel()}
        </Button>
        <Button onPress={() => save(selected)} isDisabled={change.isPending}>
          {m.front_save()}
        </Button>
      </div>
    </>
  );
}
