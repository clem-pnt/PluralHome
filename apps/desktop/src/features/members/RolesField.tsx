import { useState } from "react";
import { TextField as AriaTextField, Input, Label } from "react-aria-components";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";

const MAX_ROLES = 12;
const MAX_ROLE = 40;

/** Libellés courants ; chacun reste libre d'écrire les siens. */
const SUGGESTIONS = [
  m.role_host,
  m.role_protector,
  m.role_gatekeeper,
  m.role_little,
  m.role_caretaker,
  m.role_trauma,
  m.role_introject,
  m.role_fragment,
] as const;

const same = (a: string, b: string) => a.toLocaleLowerCase() === b.toLocaleLowerCase();

export function RolesField({
  roles,
  onChange,
}: {
  roles: string[];
  onChange: (roles: string[]) => void;
}) {
  const [draft, setDraft] = useState("");
  const full = roles.length >= MAX_ROLES;
  const add = (role: string) => {
    const r = role.trim();
    if (!r || full || roles.some((x) => same(x, r))) return;
    onChange([...roles, r]);
  };
  const suggestions = SUGGESTIONS.map((s) => s()).filter((s) => !roles.some((r) => same(r, s)));

  return (
    <div className="field">
      <span className="field__label" id="roles-label">
        {m.member_roles()}
      </span>
      {roles.length > 0 && (
        <ul className="chips" aria-labelledby="roles-label">
          {roles.map((r) => (
            <li key={r} className="chip">
              {r}
              <Button
                variant="ghost"
                aria-label={m.member_role_remove({ role: r })}
                onPress={() => onChange(roles.filter((x) => x !== r))}
              >
                ×
              </Button>
            </li>
          ))}
        </ul>
      )}
      {!full && (
        <div className="roles__add">
          <AriaTextField
            className="field roles__input"
            maxLength={MAX_ROLE}
            value={draft}
            onChange={setDraft}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                // Entrée ajoute le rôle au lieu d'envoyer le formulaire.
                e.preventDefault();
                add(draft);
                setDraft("");
              }
            }}
          >
            <Label className="sr-only">{m.member_role_new()}</Label>
            <Input className="input" placeholder={m.member_role_placeholder()} />
          </AriaTextField>
          <Button
            variant="ghost"
            isDisabled={!draft.trim()}
            onPress={() => {
              add(draft);
              setDraft("");
            }}
          >
            {m.member_role_add()}
          </Button>
        </div>
      )}
      {!full && suggestions.length > 0 && (
        <div className="chips chips--suggest">
          {suggestions.map((s) => (
            <Button key={s} variant="ghost" onPress={() => add(s)}>
              + {s}
            </Button>
          ))}
        </div>
      )}
    </div>
  );
}
