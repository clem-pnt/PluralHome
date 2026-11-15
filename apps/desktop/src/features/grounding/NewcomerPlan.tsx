import { useMutation, useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { TextField as AriaTextField, Form, Label, TextArea } from "react-aria-components";
import { keys, queryClient } from "../../app/query";
import { errorMessage } from "../../i18n/errors";
import { api, type NewcomerPlan as Plan } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { IconButton } from "../../ui/IconButton";
import { TextField } from "../../ui/TextField";

const MAX_SECTIONS = 20;

const isEmpty = (p: Plan) => !p.welcome && p.sections.length === 0;

/** Catégories proposées à la première préparation, toutes renommables. */
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
          {plan.data.sections.map((s, i) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: deux catégories peuvent porter le même titre.
            <section key={i} className="plan__section">
              <h3>{s.title}</h3>
              {s.items.length > 0 && (
                <ul>
                  {s.items.map((item) => (
                    <li key={item} data-selectable>
                      {item}
                    </li>
                  ))}
                </ul>
              )}
            </section>
          ))}
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

type Draft = { key: number; title: string; text: string };
let nextKey = 0;
const draft = (title: string, items: string[] = []): Draft => ({
  key: nextKey++,
  title,
  text: items.join("\n"),
});

function PlanForm({ plan, onDone }: { plan: Plan; onDone: () => void }) {
  const [welcome, setWelcome] = useState(plan.welcome);
  // Aucune catégorie imposée : chacun crée les siennes.
  const [sections, setSections] = useState<Draft[]>(() =>
    plan.sections.map((s) => draft(s.title, s.items)),
  );
  const save = useMutation({
    mutationFn: api.newcomer.set,
    onSuccess: (saved) => {
      queryClient.setQueryData(keys.newcomerPlan, saved);
      onDone();
    },
  });
  const patch = (key: number, p: Partial<Draft>) =>
    setSections((ss) => ss.map((s) => (s.key === key ? { ...s, ...p } : s)));
  const move = (index: number, by: number) =>
    setSections((ss) => {
      const next = [...ss];
      const [s] = next.splice(index, 1);
      if (s) next.splice(index + by, 0, s);
      return next;
    });

  return (
    <Form
      className="plan plan--edit"
      onSubmit={(e) => {
        e.preventDefault();
        save.mutate({
          welcome,
          sections: sections.map((s) => ({ title: s.title, items: s.text.split("\n") })),
        });
      }}
    >
      <AriaTextField className="field" value={welcome} onChange={setWelcome} maxLength={2000}>
        <Label className="field__label">{m.newcomer_welcome()}</Label>
        <TextArea className="input input--area" rows={4} />
        <span className="field__hint">{m.newcomer_welcome_hint()}</span>
      </AriaTextField>
      {sections.map((s, i) => (
        <fieldset key={s.key} className="plan__section-edit">
          <div className="plan__section-head">
            <TextField
              label={m.newcomer_section_title()}
              maxLength={100}
              value={s.title}
              onChange={(title) => patch(s.key, { title })}
            />
            <IconButton
              icon="close"
              label={m.newcomer_section_remove({ title: s.title || m.newcomer_section_untitled() })}
              onPress={() => setSections((ss) => ss.filter((x) => x.key !== s.key))}
            />
          </div>
          <AriaTextField
            className="field"
            value={s.text}
            onChange={(text) => patch(s.key, { text })}
            aria-label={m.newcomer_section_items({
              title: s.title || m.newcomer_section_untitled(),
            })}
          >
            <TextArea className="input input--area" rows={3} />
            <span className="field__hint">{m.newcomer_section_hint()}</span>
          </AriaTextField>
          {sections.length > 1 && (
            <div className="plan__section-move">
              <Button variant="ghost" isDisabled={i === 0} onPress={() => move(i, -1)}>
                {m.newcomer_section_up()}
              </Button>
              <Button
                variant="ghost"
                isDisabled={i === sections.length - 1}
                onPress={() => move(i, 1)}
              >
                {m.newcomer_section_down()}
              </Button>
            </div>
          )}
        </fieldset>
      ))}
      {sections.length < MAX_SECTIONS && (
        <div>
          <Button variant="ghost" onPress={() => setSections((ss) => [...ss, draft("")])}>
            {m.newcomer_section_add()}
          </Button>
        </div>
      )}
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
