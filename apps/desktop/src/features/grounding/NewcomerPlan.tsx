import { useMutation, useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { TextField as AriaTextField, Form, Label, TextArea } from "react-aria-components";
import { keys, queryClient } from "../../app/query";
import { errorMessage } from "../../i18n/errors";
import { api, type NewcomerPlan as Plan } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";

const LISTS = [
  { key: "basics", title: m.newcomer_basics, hint: m.newcomer_basics_hint },
  { key: "safe", title: m.newcomer_safe, hint: m.newcomer_safe_hint },
  { key: "steps", title: m.newcomer_steps, hint: m.newcomer_steps_hint },
] as const;

const isEmpty = (p: Plan) => !p.welcome && p.basics.length + p.safe.length + p.steps.length === 0;

/** Ce qu'un alter qui arrive au front pour la première fois a besoin de lire. */
export function NewcomerPlan() {
  const plan = useQuery({ queryKey: keys.newcomerPlan, queryFn: api.newcomer.get });
  const [editing, setEditing] = useState(false);
  if (!plan.data) return null;

  if (editing) return <PlanForm plan={plan.data} onDone={() => setEditing(false)} />;

  return (
    <div className="plan">
      {isEmpty(plan.data) ? (
        <p className="muted">{m.newcomer_empty()}</p>
      ) : (
        <>
          {plan.data.welcome && (
            <p className="plan__welcome" data-selectable>
              {plan.data.welcome}
            </p>
          )}
          {LISTS.map(
            (l) =>
              plan.data[l.key].length > 0 && (
                <section key={l.key} className="plan__section">
                  <h3>{l.title()}</h3>
                  <ul>
                    {plan.data[l.key].map((item) => (
                      <li key={item} data-selectable>
                        {item}
                      </li>
                    ))}
                  </ul>
                </section>
              ),
          )}
        </>
      )}
      <div>
        <Button variant="ghost" onPress={() => setEditing(true)}>
          {isEmpty(plan.data) ? m.newcomer_create() : m.plan_edit()}
        </Button>
      </div>
    </div>
  );
}

const toText = (items: string[]) => items.join("\n");
const toItems = (text: string) => text.split("\n");

function PlanForm({ plan, onDone }: { plan: Plan; onDone: () => void }) {
  const [welcome, setWelcome] = useState(plan.welcome);
  const [lists, setLists] = useState({
    basics: toText(plan.basics),
    safe: toText(plan.safe),
    steps: toText(plan.steps),
  });
  const save = useMutation({
    mutationFn: api.newcomer.set,
    onSuccess: (saved) => {
      queryClient.setQueryData(keys.newcomerPlan, saved);
      onDone();
    },
  });

  return (
    <Form
      className="plan plan--edit"
      onSubmit={(e) => {
        e.preventDefault();
        save.mutate({
          welcome,
          basics: toItems(lists.basics),
          safe: toItems(lists.safe),
          steps: toItems(lists.steps),
        });
      }}
    >
      <AriaTextField className="field" value={welcome} onChange={setWelcome} maxLength={2000}>
        <Label className="field__label">{m.newcomer_welcome()}</Label>
        <TextArea className="input input--area" rows={4} />
        <span className="field__hint">{m.newcomer_welcome_hint()}</span>
      </AriaTextField>
      {LISTS.map((l) => (
        <AriaTextField
          key={l.key}
          className="field"
          value={lists[l.key]}
          onChange={(v) => setLists((s) => ({ ...s, [l.key]: v }))}
        >
          <Label className="field__label">{l.title()}</Label>
          <TextArea className="input input--area" rows={3} />
          <span className="field__hint">{l.hint()}</span>
        </AriaTextField>
      ))}
      {save.isError && (
        <p className="alert" role="alert">
          {errorMessage(save.error)}
        </p>
      )}
      <div className="dialog__actions">
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
