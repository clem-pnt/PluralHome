import { useMemo, useState } from "react";
import {
  Checkbox,
  CheckboxGroup,
  Dialog,
  Heading,
  Modal,
  ModalOverlay,
  Radio,
  RadioGroup,
} from "react-aria-components";
import { errorMessage } from "../../i18n/errors";
import type { FrontSession, FrontStatus } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Avatar } from "../../ui/Avatar";
import { Button } from "../../ui/Button";
import { IconButton } from "../../ui/IconButton";
import { matches, Search } from "../../ui/Search";
import { PAGE } from "../members/MemberPicker";
import { useMembers } from "../members/useMembers";
import { STATUSES, statusLabel, useFrontChange } from "./useFront";

const BLURRY = "__blurry__";

export function FrontDialog({ current }: { current: FrontSession[] }) {
  return (
    <ModalOverlay className="overlay" isDismissable>
      <Modal className="modal modal--wide">
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
  const [statuses, setStatuses] = useState<Record<string, FrontStatus>>(() =>
    Object.fromEntries(
      current.flatMap((s) => (s.memberId ? [[s.memberId, s.status] as const] : [])),
    ),
  );
  const [q, setQ] = useState("");
  const [limit, setLimit] = useState(PAGE);
  const found = useMemo(() => members.list.filter((x) => matches(q, x)), [members.list, q]);
  const present = selected.filter((id) => id !== BLURRY && members.byId.has(id));

  const save = (values: string[]) => {
    const ids = values.filter((v) => v !== BLURRY);
    change.mutate(
      {
        members: ids,
        blurry: values.includes(BLURRY),
        statuses: Object.fromEntries(ids.map((id) => [id, statuses[id] ?? "front"])),
      },
      { onSuccess: onDone },
    );
  };

  return (
    <>
      <div>
        <Heading slot="title">{m.front_dialog_title()}</Heading>
        <p className="muted">{m.front_dialog_lead()}</p>
      </div>

      {present.length > 0 && (
        <section className="present" aria-label={m.front_present({ count: present.length })}>
          <h3 className="present__title">{m.front_present({ count: present.length })}</h3>
          <ul className="present__list">
            {present.map((id) => {
              const member = members.byId.get(id);
              if (!member) return null;
              return (
                <li key={id} className="present__row">
                  <Avatar name={member.name} src={member.avatar} size={28} />
                  <span className="present__name">{member.name}</span>
                  <RadioGroup
                    className="seg"
                    orientation="horizontal"
                    aria-label={m.front_status_of({ name: member.name })}
                    value={statuses[id] ?? "front"}
                    onChange={(v) => setStatuses((s) => ({ ...s, [id]: v as FrontStatus }))}
                  >
                    {STATUSES.map((st) => (
                      <Radio key={st} value={st} className="seg__item">
                        {statusLabel(st)}
                      </Radio>
                    ))}
                  </RadioGroup>
                  <IconButton
                    icon="close"
                    label={m.action_remove()}
                    onPress={() => setSelected((s) => s.filter((x) => x !== id))}
                  />
                </li>
              );
            })}
          </ul>
        </section>
      )}

      <div className="field">
        {members.list.length > 8 && (
          <Search
            label={m.search_members()}
            value={q}
            onChange={(v) => {
              setQ(v);
              setLimit(PAGE);
            }}
            autoFocus
          />
        )}
        <CheckboxGroup
          className="picker"
          aria-label={m.front_members()}
          value={selected}
          // Le groupe garde aussi les membres cochés que la recherche masque.
          onChange={(ids) => setSelected([...new Set(ids)])}
        >
          {found.slice(0, limit).map((member) => (
            <Checkbox key={member.id} value={member.id} className="pick">
              <Avatar name={member.name} src={member.avatar} size={32} />
              <span className="pick__name">{member.name}</span>
            </Checkbox>
          ))}
          {!q.trim() && (
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
          )}
        </CheckboxGroup>
        {found.length === 0 && <p className="muted">{m.search_none()}</p>}
        {found.length > limit && (
          <Button variant="ghost" onPress={() => setLimit((n) => n + PAGE * 2)}>
            {m.search_more({ count: found.length - limit })}
          </Button>
        )}
      </div>

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
