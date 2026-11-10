import { useMutation, useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { TextField as AriaTextField, Form, Label, TextArea } from "react-aria-components";
import { keys, queryClient } from "../../app/query";
import { errorMessage } from "../../i18n/errors";
import { api, type Contact, type SafetyPlan as Plan } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { TextField } from "../../ui/TextField";

const LISTS = [
  { key: "warningSigns", title: m.plan_warning_signs, hint: m.plan_warning_signs_hint },
  { key: "coping", title: m.plan_coping, hint: m.plan_coping_hint },
  { key: "reasons", title: m.plan_reasons, hint: m.plan_reasons_hint },
] as const;

const isEmpty = (p: Plan) =>
  p.warningSigns.length + p.coping.length + p.reasons.length + p.contacts.length === 0;

/** Plan de crise personnel (coffre déverrouillé uniquement). */
export function SafetyPlan() {
  const plan = useQuery({ queryKey: keys.safetyPlan, queryFn: api.safety.get });
  const [editing, setEditing] = useState(false);
  if (!plan.data) return null;

  if (editing) return <PlanForm plan={plan.data} onDone={() => setEditing(false)} />;

  return (
    <div className="plan">
      {isEmpty(plan.data) ? (
        <p className="muted">{m.plan_empty()}</p>
      ) : (
        <>
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
          {plan.data.contacts.length > 0 && (
            <section className="plan__section">
              <h3>{m.plan_contacts()}</h3>
              <ul className="plan__contacts">
                {plan.data.contacts.map((c) => (
                  <li key={`${c.name}-${c.phone}`} data-selectable>
                    <strong>{c.name}</strong>
                    {c.phone && <span className="line__number line__number--small">{c.phone}</span>}
                  </li>
                ))}
              </ul>
            </section>
          )}
        </>
      )}
      <div>
        <Button variant="ghost" onPress={() => setEditing(true)}>
          {isEmpty(plan.data) ? m.plan_create() : m.plan_edit()}
        </Button>
      </div>
    </div>
  );
}

const toText = (items: string[]) => items.join("\n");
const toItems = (text: string) => text.split("\n");

function PlanForm({ plan, onDone }: { plan: Plan; onDone: () => void }) {
  const [lists, setLists] = useState({
    warningSigns: toText(plan.warningSigns),
    coping: toText(plan.coping),
    reasons: toText(plan.reasons),
  });
  const [contacts, setContacts] = useState<Contact[]>(
    plan.contacts.length > 0 ? plan.contacts : [{ name: "", phone: null }],
  );
  const save = useMutation({
    mutationFn: api.safety.set,
    onSuccess: (saved) => {
      queryClient.setQueryData(keys.safetyPlan, saved);
      onDone();
    },
  });
  const setContact = (i: number, patch: Partial<Contact>) =>
    setContacts((cs) => cs.map((c, j) => (j === i ? { ...c, ...patch } : c)));

  return (
    <Form
      className="plan plan--edit"
      onSubmit={(e) => {
        e.preventDefault();
        save.mutate({
          warningSigns: toItems(lists.warningSigns),
          coping: toItems(lists.coping),
          reasons: toItems(lists.reasons),
          contacts,
        });
      }}
    >
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
      <fieldset className="plan__contacts-edit">
        <legend className="field__label">{m.plan_contacts()}</legend>
        {contacts.map((c, i) => (
          // biome-ignore lint/suspicious/noArrayIndexKey: lignes éditables sans identifiant propre
          <div key={i} className="plan__contact-row">
            <TextField
              label={m.plan_contact_name()}
              maxLength={100}
              value={c.name}
              onChange={(name) => setContact(i, { name })}
            />
            <TextField
              label={m.plan_contact_phone()}
              type="tel"
              maxLength={32}
              value={c.phone ?? ""}
              onChange={(phone) => setContact(i, { phone: phone || null })}
            />
          </div>
        ))}
        {contacts.length < 20 && (
          <div>
            <Button
              variant="ghost"
              onPress={() => setContacts((cs) => [...cs, { name: "", phone: null }])}
            >
              {m.plan_contact_add()}
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
