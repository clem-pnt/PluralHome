import { useRef, useState } from "react";
import {
  Dialog,
  DialogTrigger,
  Form,
  Heading,
  Modal,
  ModalOverlay,
  Switch,
} from "react-aria-components";
import { errorMessage } from "../../i18n/errors";
import { formatDeadline } from "../../i18n/format";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { IconButton } from "../../ui/IconButton";
import { TextField } from "../../ui/TextField";
import { useCreatePoll } from "./usePolls";

const MAX_OPTIONS = 20;

/**
 * Échéance choisie : date et heure, ou la date seule (fin de journée), ou
 * l'heure seule (aujourd'hui, demain si elle est passée). `null` sans rien.
 */
export function endOf(date: string, time: string, now = Date.now()): number | null {
  if (!date && !time) return null;
  const [h, min] = time ? time.split(":").map(Number) : [23, 59];
  const d = date ? new Date(`${date}T00:00`) : new Date(now);
  d.setHours(h ?? 0, min ?? 0, 0, 0);
  if (!date && d.getTime() <= now) d.setDate(d.getDate() + 1);
  return Number.isNaN(d.getTime()) ? null : d.getTime();
}

export function NewPollDialog({ author }: { author: string | null }) {
  return (
    <DialogTrigger>
      <Button>{m.vote_new()}</Button>
      <ModalOverlay className="overlay" isDismissable>
        <Modal className="modal modal--wide">
          <Dialog className="dialog">
            {({ close }) => <NewPollForm author={author} onDone={close} />}
          </Dialog>
        </Modal>
      </ModalOverlay>
    </DialogTrigger>
  );
}

function NewPollForm({ author, onDone }: { author: string | null; onDone: () => void }) {
  const create = useCreatePoll();
  const next = useRef(2);
  const [question, setQuestion] = useState("");
  const [options, setOptions] = useState([
    { key: 0, text: "" },
    { key: 1, text: "" },
  ]);
  const [multi, setMulti] = useState(false);
  const [hasEnd, setHasEnd] = useState(false);
  const [endDate, setEndDate] = useState("");
  const [endTime, setEndTime] = useState("");
  const endsAt = hasEnd ? endOf(endDate, endTime) : null;
  const endPast = endsAt != null && endsAt <= Date.now();
  const filled = options.filter((o) => o.text.trim()).length;
  const ready = question.trim() !== "" && filled >= 2 && (!hasEnd || (endsAt != null && !endPast));

  return (
    <Form
      className="poll-form"
      onSubmit={(e) => {
        e.preventDefault();
        if (!ready || create.isPending) return;
        create.mutate(
          { question, authorId: author, multi, options: options.map((o) => o.text), endsAt },
          { onSuccess: onDone },
        );
      }}
    >
      <div>
        <Heading slot="title">{m.vote_new()}</Heading>
        <p className="muted">{m.vote_new_lead()}</p>
      </div>
      <TextField
        label={m.vote_question()}
        value={question}
        onChange={setQuestion}
        maxLength={300}
        placeholder={m.vote_question_placeholder()}
        autoFocus
      />
      <fieldset className="poll-form__options">
        <legend className="field__label">{m.vote_options()}</legend>
        {options.map((o, i) => (
          <div key={o.key} className="poll-form__option">
            <TextField
              label={m.vote_option_n({ n: i + 1 })}
              value={o.text}
              maxLength={100}
              onChange={(text) =>
                setOptions((all) => all.map((x) => (x.key === o.key ? { ...x, text } : x)))
              }
            />
            <IconButton
              icon="close"
              label={m.vote_option_remove()}
              isDisabled={options.length <= 2}
              onPress={() => setOptions((all) => all.filter((x) => x.key !== o.key))}
            />
          </div>
        ))}
        <Button
          variant="ghost"
          isDisabled={options.length >= MAX_OPTIONS}
          onPress={() => {
            const key = next.current++;
            setOptions((all) => [...all, { key, text: "" }]);
          }}
        >
          {m.vote_option_add()}
        </Button>
      </fieldset>
      <Switch className="choice" isSelected={multi} onChange={setMulti}>
        {m.vote_multi()}
      </Switch>
      <Switch className="choice" isSelected={hasEnd} onChange={setHasEnd}>
        {m.vote_end_toggle()}
      </Switch>
      {hasEnd && (
        <div className="poll-form__end">
          <div className="poll-form__end-fields">
            <label className="field">
              <span className="field__label">{m.vote_end_date()}</span>
              <input
                className="input"
                type="date"
                value={endDate}
                onChange={(e) => setEndDate(e.target.value)}
              />
            </label>
            <label className="field">
              <span className="field__label">{m.vote_end_time()}</span>
              <input
                className="input"
                type="time"
                value={endTime}
                onChange={(e) => setEndTime(e.target.value)}
              />
            </label>
          </div>
          <p className={endPast ? "field__error" : "field__hint"}>
            {endsAt == null
              ? m.vote_end_hint()
              : endPast
                ? m.vote_end_past()
                : m.vote_ends_on({ when: formatDeadline(endsAt) })}
          </p>
        </div>
      )}
      {create.isError && (
        <p className="alert" role="alert">
          {errorMessage(create.error)}
        </p>
      )}
      <div className="dialog__actions">
        <Button variant="ghost" slot="close">
          {m.cancel()}
        </Button>
        <Button type="submit" isDisabled={!ready || create.isPending}>
          {m.vote_create()}
        </Button>
      </div>
    </Form>
  );
}
