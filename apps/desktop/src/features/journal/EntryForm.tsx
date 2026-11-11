import { useState } from "react";
import {
  TextField as AriaTextField,
  Checkbox,
  CheckboxGroup,
  Form,
  Label,
  Radio,
  RadioGroup,
  TextArea,
} from "react-aria-components";
import { errorMessage } from "../../i18n/errors";
import type { Entry } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Avatar } from "../../ui/Avatar";
import { Button } from "../../ui/Button";
import { TextField } from "../../ui/TextField";
import { useFrontCurrent } from "../front/useFront";
import { useMembers } from "../members/useMembers";
import { MOODS } from "./mood";
import { useSaveEntry } from "./useJournal";

const MAX_TITLE = 120;
const MAX_BODY = 20000;
const NO_MOOD = "none";

/** Écriture d'une nouvelle entrée (`entry` absent) ou modification. */
export function EntryForm({ entry, onDone }: { entry?: Entry; onDone?: () => void }) {
  const members = useMembers();
  const front = useFrontCurrent();
  const save = useSaveEntry();
  // Par défaut : les personnes au front signent l'entrée.
  const fronting = (front.data ?? []).flatMap((s) => (s.memberId ? [s.memberId] : []));
  const [authors, setAuthors] = useState<string[] | null>(entry ? entry.authorIds : null);
  const [title, setTitle] = useState(entry?.title ?? "");
  const [body, setBody] = useState(entry?.body ?? "");
  const [mood, setMood] = useState<string>(entry?.mood ? String(entry.mood) : NO_MOOD);
  const signed = authors ?? fronting;

  const submit = () => {
    if (!body.trim() || save.isPending) return;
    save.mutate(
      {
        id: entry?.id ?? null,
        input: {
          title: title.trim() || null,
          body,
          mood: mood === NO_MOOD ? null : Number(mood),
          authorIds: signed,
        },
      },
      {
        onSuccess: () => {
          if (!entry) {
            setTitle("");
            setBody("");
            setMood(NO_MOOD);
            setAuthors(null);
          }
          onDone?.();
        },
      },
    );
  };

  return (
    <Form
      className={entry ? "journal-form" : "panel journal-form"}
      onSubmit={(e) => {
        e.preventDefault();
        submit();
      }}
    >
      <TextField
        label={m.journal_entry_title()}
        hint={m.journal_entry_title_hint()}
        maxLength={MAX_TITLE}
        value={title}
        onChange={setTitle}
      />
      <AriaTextField className="field" value={body} onChange={setBody} maxLength={MAX_BODY}>
        <Label className="field__label">{m.journal_entry_body()}</Label>
        <TextArea
          className="input input--area"
          rows={entry ? 8 : 4}
          placeholder={m.journal_entry_placeholder()}
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
              e.preventDefault();
              submit();
            }
          }}
        />
      </AriaTextField>
      {members.list.length > 0 && (
        <CheckboxGroup className="field" value={signed} onChange={setAuthors}>
          <Label className="field__label">{m.journal_authors()}</Label>
          <div className="choices">
            {members.list.map((x) => (
              <Checkbox key={x.id} value={x.id} className="choice">
                <Avatar name={x.name} color={x.color} src={x.avatar} size={22} />
                {x.name}
              </Checkbox>
            ))}
          </div>
          <span className="field__hint">{m.journal_authors_hint()}</span>
        </CheckboxGroup>
      )}
      <RadioGroup className="field" value={mood} onChange={setMood} orientation="horizontal">
        <Label className="field__label">{m.journal_mood()}</Label>
        <div className="choices">
          <Radio value={NO_MOOD} className="choice">
            {m.journal_mood_none()}
          </Radio>
          {MOODS.map((x) => (
            <Radio key={x.value} value={String(x.value)} className="choice" data-mood={x.value}>
              {x.label()}
            </Radio>
          ))}
        </div>
      </RadioGroup>
      {save.isError && (
        <p className="alert" role="alert">
          {errorMessage(save.error)}
        </p>
      )}
      <div className="composer__actions">
        <span className="muted">{m.messages_send_hint()}</span>
        <div className="panel__actions">
          {onDone && entry && (
            <Button variant="ghost" onPress={onDone}>
              {m.cancel()}
            </Button>
          )}
          <Button type="submit" isDisabled={!body.trim() || save.isPending}>
            {entry ? m.journal_save() : m.journal_write()}
          </Button>
        </div>
      </div>
    </Form>
  );
}
