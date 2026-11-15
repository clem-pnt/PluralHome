import { useMutation, useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { Form } from "react-aria-components";
import { keys, queryClient } from "../../app/query";
import { errorMessage } from "../../i18n/errors";
import { api, type ContactChannel, type TrustedContact as Trusted } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { IconButton } from "../../ui/IconButton";
import { Select } from "../../ui/Select";
import { TextField } from "../../ui/TextField";

const KINDS = [
  { id: "phone", label: m.channel_phone },
  { id: "discord", label: m.channel_discord },
  { id: "instagram", label: m.channel_instagram },
  { id: "snapchat", label: m.channel_snapchat },
  { id: "whatsapp", label: m.channel_whatsapp },
  { id: "signal", label: m.channel_signal },
  { id: "telegram", label: m.channel_telegram },
  { id: "messenger", label: m.channel_messenger },
  { id: "email", label: m.channel_email },
  { id: "other", label: m.channel_other },
] as const;
const MAX_CHANNELS = 10;

export const channelLabel = (kind: string) =>
  (KINDS.find((k) => k.id === kind) ?? KINDS[KINDS.length - 1])?.label() ?? kind;

/** Un proche choisi par le système, avec tous les moyens de le joindre, en tête des numéros d'aide. */
export function TrustedContact() {
  const trusted = useQuery({ queryKey: keys.trusted, queryFn: api.trusted.get });
  const [editing, setEditing] = useState(false);
  if (trusted.isPending) return null;
  const contact = trusted.data ?? null;

  if (editing) return <TrustedForm contact={contact} onDone={() => setEditing(false)} />;

  if (!contact)
    return (
      <div className="line line--trusted line--empty">
        <span className="line__text">
          <strong>{m.trusted_title()}</strong>
          <span className="muted">{m.trusted_empty()}</span>
        </span>
        <Button variant="ghost" onPress={() => setEditing(true)}>
          {m.trusted_add()}
        </Button>
      </div>
    );

  return (
    <div className="line line--trusted">
      <span className="line__text">
        <strong>{contact.name}</strong>
        <span className="muted">{m.trusted_title()}</span>
      </span>
      <ul className="channels">
        {contact.channels.map((c) => (
          <li key={`${c.kind}:${c.value}`}>
            <span className="channels__kind muted">{channelLabel(c.kind)}</span>
            {c.kind === "email" ? (
              <button
                type="button"
                className="channels__value channels__value--link"
                onClick={() => void api.mailto(c.value).catch(() => {})}
              >
                {c.value}
              </button>
            ) : (
              <span className="channels__value" data-selectable>
                {c.value}
              </span>
            )}
          </li>
        ))}
      </ul>
      <IconButton icon="edit" label={m.trusted_edit()} onPress={() => setEditing(true)} />
    </div>
  );
}

type Row = ContactChannel & { key: number };
let nextKey = 0;
const row = (c: ContactChannel): Row => ({ ...c, key: nextKey++ });

function TrustedForm({ contact, onDone }: { contact: Trusted | null; onDone: () => void }) {
  const [name, setName] = useState(contact?.name ?? "");
  const [rows, setRows] = useState<Row[]>(() =>
    contact?.channels.length
      ? contact.channels.map((c) => row(c.kind === "sms" ? { ...c, kind: "phone" } : c))
      : [row({ kind: "phone", value: "" })],
  );
  const save = useMutation({
    mutationFn: api.trusted.set,
    onSuccess: (saved) => {
      queryClient.setQueryData(keys.trusted, saved);
      onDone();
    },
  });
  const patch = (key: number, p: Partial<ContactChannel>) =>
    setRows((rs) => rs.map((r) => (r.key === key ? { ...r, ...p } : r)));

  const channels = rows.map(({ kind, value }) => ({ kind, value }));
  const empty = !name.trim() && channels.every((c) => !c.value.trim());

  return (
    <Form
      className="trusted-form panel"
      onSubmit={(e) => {
        e.preventDefault();
        save.mutate(empty ? null : { name, channels });
      }}
    >
      <h3>{m.trusted_title()}</h3>
      <p className="muted">{m.trusted_hint()}</p>
      <TextField
        label={m.plan_contact_name()}
        maxLength={100}
        value={name}
        onChange={setName}
        autoFocus
      />
      <fieldset className="channels-edit">
        <legend className="field__label">{m.channel_title()}</legend>
        {rows.map((r) => (
          <div key={r.key} className="channels-edit__row">
            <Select
              label={m.channel_kind()}
              hideLabel
              options={KINDS.map((k) => ({ id: k.id, label: k.label() }))}
              value={r.kind}
              onChange={(kind) => patch(r.key, { kind })}
            />
            <TextField
              label={m.channel_value()}
              hideLabel
              maxLength={100}
              placeholder={
                r.kind === "phone" || r.kind === "whatsapp"
                  ? "06 12 34 56 78"
                  : r.kind === "email"
                    ? "nom@exemple.fr"
                    : m.channel_handle_placeholder()
              }
              value={r.value}
              onChange={(value) => patch(r.key, { value })}
            />
            <IconButton
              icon="close"
              label={m.channel_remove()}
              isDisabled={rows.length === 1}
              onPress={() => setRows((rs) => rs.filter((x) => x.key !== r.key))}
            />
          </div>
        ))}
        {rows.length < MAX_CHANNELS && (
          <div>
            <Button
              variant="ghost"
              onPress={() => setRows((rs) => [...rs, row({ kind: "discord", value: "" })])}
            >
              {m.channel_add()}
            </Button>
          </div>
        )}
      </fieldset>
      {save.isError && (
        <p className="alert" role="alert">
          {errorMessage(save.error)}
        </p>
      )}
      <div className="dialog__actions">
        {contact && (
          <Button variant="ghost" onPress={() => save.mutate(null)}>
            {m.trusted_remove()}
          </Button>
        )}
        <Button variant="ghost" onPress={onDone}>
          {m.cancel()}
        </Button>
        <Button type="submit" isDisabled={save.isPending}>
          {m.plan_save()}
        </Button>
      </div>
    </Form>
  );
}
