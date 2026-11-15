import { useState } from "react";
import { Form, Label, TextArea, TextField } from "react-aria-components";
import { errorMessage } from "../../i18n/errors";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { useFrontCurrent } from "../front/useFront";
import { useMembers } from "../members/useMembers";
import { PeopleField } from "./PeopleField";
import { fronting } from "./people";
import { usePostMessage } from "./useMessages";

export const MAX_BODY = 4000;

export function Composer() {
  const members = useMembers();
  const front = useFrontCurrent();
  const post = usePostMessage();

  // Par défaut, ce sont les personnes au front qui écrivent, tant qu'on n'a pas choisi.
  const [authors, setAuthors] = useState<string[] | null>(null);
  const [recipients, setRecipients] = useState<string[]>([]);
  const [body, setBody] = useState("");
  const from = authors ?? fronting(front.data);

  const send = () => {
    if (!body.trim() || post.isPending) return;
    post.mutate(
      { authorIds: from, recipientIds: recipients, body },
      { onSuccess: () => setBody("") },
    );
  };

  return (
    <Form
      className="panel composer"
      onSubmit={(e) => {
        e.preventDefault();
        send();
      }}
    >
      <div className="composer__route">
        <PeopleField
          label={m.messages_from()}
          title={m.messages_pick_authors()}
          lead={m.messages_pick_authors_lead()}
          emptyLabel={m.messages_pick_none_author()}
          members={members.list}
          byId={members.byId}
          value={from}
          onChange={setAuthors}
        />
        <PeopleField
          label={m.messages_to()}
          title={m.messages_pick_recipients()}
          lead={m.messages_pick_recipients_lead()}
          emptyLabel={m.messages_everyone()}
          members={members.list}
          byId={members.byId}
          value={recipients}
          onChange={setRecipients}
        />
      </div>
      <TextField className="field" value={body} onChange={setBody} maxLength={MAX_BODY}>
        <Label className="field__label">{m.messages_body()}</Label>
        <TextArea
          className="input textarea"
          rows={3}
          placeholder={m.messages_placeholder()}
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
              e.preventDefault();
              send();
            }
          }}
        />
      </TextField>
      {post.isError && (
        <p className="alert" role="alert">
          {errorMessage(post.error)}
        </p>
      )}
      <div className="composer__actions">
        <span className="muted">{m.messages_send_hint()}</span>
        <Button type="submit" isDisabled={!body.trim() || post.isPending}>
          {m.messages_send()}
        </Button>
      </div>
    </Form>
  );
}
