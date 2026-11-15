import { useState } from "react";
import { Heading, Label, TextArea, TextField } from "react-aria-components";
import { errorMessage } from "../../i18n/errors";
import type { Member, Message } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { PeopleField } from "./PeopleField";
import { MAX_BODY, useUpdateMessage } from "./useMessages";

/** Contenu de la fenêtre « Modifier le message » : texte, auteurs et destinataires. */
export function EditMessageForm({
  message,
  members,
  byId,
  onDone,
}: {
  message: Message;
  members: Member[];
  byId: Map<string, Member>;
  onDone: () => void;
}) {
  const update = useUpdateMessage();
  const [authors, setAuthors] = useState(message.authorIds);
  const [recipients, setRecipients] = useState(message.recipientIds);
  const [body, setBody] = useState(message.body);
  const save = () =>
    update.mutate(
      { id: message.id, input: { authorIds: authors, recipientIds: recipients, body } },
      { onSuccess: onDone },
    );

  return (
    <>
      <Heading slot="title">{m.messages_edit_title()}</Heading>
      <div className="composer__route">
        <PeopleField
          label={m.messages_from()}
          title={m.messages_pick_authors()}
          lead={m.messages_pick_authors_lead()}
          emptyLabel={m.messages_pick_none_author()}
          members={members}
          byId={byId}
          value={authors}
          onChange={setAuthors}
        />
        <PeopleField
          label={m.messages_to()}
          title={m.messages_pick_recipients()}
          lead={m.messages_pick_recipients_lead()}
          emptyLabel={m.messages_everyone()}
          members={members}
          byId={byId}
          value={recipients}
          onChange={setRecipients}
        />
      </div>
      <TextField className="field" value={body} onChange={setBody} maxLength={MAX_BODY} autoFocus>
        <Label className="field__label">{m.messages_body()}</Label>
        <TextArea className="input textarea" rows={5} />
      </TextField>
      {update.isError && (
        <p className="alert" role="alert">
          {errorMessage(update.error)}
        </p>
      )}
      <div className="dialog__actions">
        <Button variant="ghost" slot="close">
          {m.cancel()}
        </Button>
        <Button isDisabled={!body.trim() || update.isPending} onPress={save}>
          {m.messages_save()}
        </Button>
      </div>
    </>
  );
}
