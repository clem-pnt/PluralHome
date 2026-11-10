import { useState } from "react";
import { Form, Label, TextArea, TextField } from "react-aria-components";
import { errorMessage } from "../../i18n/errors";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { Select } from "../../ui/Select";
import { useFrontCurrent } from "../front/useFront";
import { useMembers } from "../members/useMembers";
import { usePostMessage } from "./useMessages";

const NONE = "__none__";
const MAX = 4000;

export function Composer() {
  const members = useMembers();
  const front = useFrontCurrent();
  const post = usePostMessage();

  // Par défaut, l'auteur est la personne au front si elle est seule.
  const solo = front.data?.length === 1 ? (front.data[0]?.memberId ?? null) : null;
  const [author, setAuthor] = useState<string | null>(null);
  const [recipient, setRecipient] = useState<string>(NONE);
  const [body, setBody] = useState("");
  const from = author ?? solo ?? NONE;

  const people = members.list.map((x) => ({ id: x.id, label: x.name }));
  const send = () => {
    if (!body.trim() || post.isPending) return;
    post.mutate(
      {
        authorId: from === NONE ? null : from,
        recipientId: recipient === NONE ? null : recipient,
        body,
      },
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
        <Select
          label={m.messages_from()}
          options={[{ id: NONE, label: m.messages_someone() }, ...people]}
          value={from}
          onChange={setAuthor}
        />
        <Select
          label={m.messages_to()}
          options={[{ id: NONE, label: m.messages_everyone() }, ...people]}
          value={recipient}
          onChange={setRecipient}
        />
      </div>
      <TextField className="field" value={body} onChange={setBody} maxLength={MAX}>
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
