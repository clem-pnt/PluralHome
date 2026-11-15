import { useState } from "react";
import {
  Button as AriaButton,
  Dialog,
  DialogTrigger,
  Heading,
  Modal,
  ModalOverlay,
} from "react-aria-components";
import type { Member } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Avatar } from "../../ui/Avatar";
import { Button } from "../../ui/Button";
import { Icon } from "../../ui/Icon";
import { MemberPicker } from "../members/MemberPicker";
import { names } from "./people";

/**
 * Choix de plusieurs membres (auteurs ou destinataires) : un bouton résume la
 * sélection et ouvre un sélecteur avec recherche, prévu pour les grands systèmes.
 */
export function PeopleField({
  label,
  title,
  lead,
  emptyLabel,
  members,
  byId,
  value,
  onChange,
  single,
}: {
  label: string;
  title: string;
  lead: string;
  /** Ce que veut dire une sélection vide (« Tout le système », « Sans signature »). */
  emptyLabel: string;
  members: Member[];
  byId: Map<string, Member>;
  value: string[];
  onChange: (ids: string[]) => void;
  /** Un seul membre : toucher un nom le choisit et ferme. */
  single?: boolean;
}) {
  const shown = value.flatMap((id) => {
    const x = byId.get(id);
    return x ? [x] : [];
  });
  return (
    <div className="field">
      <span className="field__label">{label}</span>
      <DialogTrigger>
        <AriaButton
          className="people-btn"
          aria-label={`${label} : ${names(value, byId) || emptyLabel}`}
        >
          {shown.length > 0 ? (
            <span className="people-btn__faces" aria-hidden>
              {shown.slice(0, 3).map((x) => (
                <Avatar key={x.id} name={x.name} src={x.avatar} size={24} />
              ))}
            </span>
          ) : (
            <span className="people-btn__faces people-btn__faces--none" aria-hidden>
              <Icon name="users" size={16} />
            </span>
          )}
          <span className="people-btn__text">{names(value, byId) || emptyLabel}</span>
          <span className="people-btn__change">{m.messages_change_people()}</span>
        </AriaButton>
        <ModalOverlay className="overlay" isDismissable>
          <Modal className="modal modal--wide">
            <Dialog className="dialog">
              {({ close }) => (
                <Picker
                  title={title}
                  lead={lead}
                  emptyLabel={emptyLabel}
                  members={members}
                  initial={value}
                  single={single ?? false}
                  onDone={(ids) => {
                    onChange(ids);
                    close();
                  }}
                />
              )}
            </Dialog>
          </Modal>
        </ModalOverlay>
      </DialogTrigger>
    </div>
  );
}

function Picker({
  title,
  lead,
  emptyLabel,
  members,
  initial,
  single,
  onDone,
}: {
  title: string;
  lead: string;
  emptyLabel: string;
  members: Member[];
  initial: string[];
  single: boolean;
  onDone: (ids: string[]) => void;
}) {
  const [value, setValue] = useState(initial);
  const pick = (ids: string[]) => {
    setValue(ids);
    if (single && ids.length > 0) onDone(ids);
  };
  return (
    <>
      <div>
        <Heading slot="title">{title}</Heading>
        <p className="muted">{lead}</p>
      </div>
      <MemberPicker
        label={title}
        members={members}
        value={value}
        onChange={pick}
        single={single}
        autoFocus
      />
      <div className="dialog__actions">
        <Button variant="ghost" onPress={() => onDone([])}>
          {emptyLabel}
        </Button>
        <span style={{ flex: 1 }} />
        <Button variant="ghost" slot="close">
          {m.cancel()}
        </Button>
        {!single && <Button onPress={() => onDone(value)}>{m.messages_pick_validate()}</Button>}
      </div>
    </>
  );
}
