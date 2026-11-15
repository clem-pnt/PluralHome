import { useMutation, useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { Form } from "react-aria-components";
import { keys, queryClient } from "../../app/query";
import { errorMessage } from "../../i18n/errors";
import { api, type Contact } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { IconButton } from "../../ui/IconButton";
import { TextField } from "../../ui/TextField";

/** Numéro d'un proche choisi par le système, en tête des numéros d'aide. */
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
      <span className="line__number" data-selectable>
        {contact.phone}
      </span>
      <span className="line__text">
        <strong>{contact.name}</strong>
        <span className="muted">{m.trusted_title()}</span>
      </span>
      <IconButton icon="edit" label={m.trusted_edit()} onPress={() => setEditing(true)} />
    </div>
  );
}

function TrustedForm({ contact, onDone }: { contact: Contact | null; onDone: () => void }) {
  const [name, setName] = useState(contact?.name ?? "");
  const [phone, setPhone] = useState(contact?.phone ?? "");
  const save = useMutation({
    mutationFn: api.trusted.set,
    onSuccess: (saved) => {
      queryClient.setQueryData(keys.trusted, saved);
      onDone();
    },
  });

  return (
    <Form
      className="trusted-form panel"
      onSubmit={(e) => {
        e.preventDefault();
        save.mutate(name.trim() || phone.trim() ? { name, phone: phone || null } : null);
      }}
    >
      <h3>{m.trusted_title()}</h3>
      <p className="muted">{m.trusted_hint()}</p>
      <div className="plan__contact-row">
        <TextField
          label={m.plan_contact_name()}
          maxLength={100}
          value={name}
          onChange={setName}
          autoFocus
        />
        <TextField
          label={m.plan_contact_phone()}
          type="tel"
          maxLength={32}
          value={phone}
          onChange={setPhone}
        />
      </div>
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
