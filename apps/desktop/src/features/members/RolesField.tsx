import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { TextField as AriaTextField, Input, Label } from "react-aria-components";
import { keys } from "../../app/query";
import { api } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";

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
  const catalog = useQuery({ queryKey: keys.roles, queryFn: api.roles.list });
  const add = (role: string) => {
    const r = role.trim();
    if (!r || roles.some((x) => same(x, r))) return;
    onChange([...roles, r]);
  };
  // Rôles courants puis ceux créés par le système, sans doublon.
  const suggestions: string[] = [];
  for (const s of [...SUGGESTIONS.map((f) => f()), ...(catalog.data ?? [])]) {
    if (!roles.some((r) => same(r, s)) && !suggestions.some((x) => same(x, s))) suggestions.push(s);
  }

  return (
    <div className="field">
      <span className="field__label" id="roles-label">
        {m.member_roles()}
      </span>
      {roles.length > 0 && (
        <ul className="chips" aria-labelledby="roles-label">
          {roles.map((r, i) => (
            <li key={r} className={i === 0 ? "chip chip--main" : "chip"}>
              {r}
              {i === 0 && roles.length > 1 && (
                <small className="chip__tag">{m.member_role_main()}</small>
              )}
              {i > 0 && (
                <Button
                  variant="ghost"
                  aria-label={m.member_role_make_main({ role: r })}
                  onPress={() => onChange([r, ...roles.filter((x) => x !== r)])}
                >
                  {m.member_role_make_main_short()}
                </Button>
              )}
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
      {roles.length > 1 && <span className="field__hint">{m.member_roles_hint()}</span>}
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
      {suggestions.length > 0 && (
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
