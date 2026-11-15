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
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { IconButton } from "../../ui/IconButton";
import { TextField } from "../../ui/TextField";
import { useCreatePoll } from "./usePolls";

const MAX_OPTIONS = 20;

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
  const filled = options.filter((o) => o.text.trim()).length;
  const ready = question.trim() !== "" && filled >= 2;

  return (
    <Form
      className="poll-form"
      onSubmit={(e) => {
        e.preventDefault();
        if (!ready || create.isPending) return;
        create.mutate(
          { question, authorId: author, multi, options: options.map((o) => o.text) },
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
