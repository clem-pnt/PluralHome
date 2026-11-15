import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import {
  TextField as AriaTextField,
  Autocomplete,
  Dialog,
  DialogTrigger,
  Input,
  Label,
  ListBox,
  ListBoxItem,
  Popover,
} from "react-aria-components";
import { keys } from "../../app/query";
import { api } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { IconButton } from "../../ui/IconButton";

const MAX_ROLE = 40;

/** Libellés courants ; chacun reste libre d'écrire les siens. */
const SUGGESTIONS = [
  m.role_host,
  m.role_cohost,
  m.role_protector,
  m.role_gatekeeper,
  m.role_little,
  m.role_caretaker,
  m.role_trauma,
  m.role_persecutor,
  m.role_introject,
  m.role_fictive,
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
      <ul className="chips" aria-labelledby="roles-label">
        {roles.map((r, i) => (
          <li key={r} className={i === 0 ? "chip chip--main" : "chip"}>
            {r}
            {i === 0 && roles.length > 1 && (
              <small className="chip__tag">{m.member_role_main()}</small>
            )}
            {i > 0 && (
              <IconButton
                icon="star"
                label={m.member_role_make_main({ role: r })}
                onPress={() => onChange([r, ...roles.filter((x) => x !== r)])}
              />
            )}
            <IconButton
              icon="close"
              label={m.member_role_remove({ role: r })}
              onPress={() => onChange(roles.filter((x) => x !== r))}
            />
          </li>
        ))}
        <li className="chips__add">
          <RolePicker suggestions={suggestions} onAdd={add} />
        </li>
      </ul>
      {roles.length > 1 && <span className="field__hint">{m.member_roles_hint()}</span>}
    </div>
  );
}

const CREATE = "\u0000create";

/** Bouton « + » qui ouvre une liste filtrable des rôles, avec saisie libre. */
function RolePicker({
  suggestions,
  onAdd,
}: {
  suggestions: string[];
  onAdd: (role: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const q = query.trim();
  const shown = suggestions.filter((s) => s.toLocaleLowerCase().includes(q.toLocaleLowerCase()));
  const items = [
    ...(q && !suggestions.some((s) => same(s, q)) ? [{ id: CREATE, label: q }] : []),
    ...shown.map((s) => ({ id: s, label: s })),
  ];
  const pick = (role: string) => {
    onAdd(role);
    setQuery("");
    setOpen(false);
  };

  return (
    <DialogTrigger
      isOpen={open}
      onOpenChange={(o) => {
        setOpen(o);
        if (!o) setQuery("");
      }}
    >
      <IconButton icon="plus" label={m.member_role_add_one()} />
      <Popover className="popover role-picker" placement="bottom start">
        <Dialog className="role-picker__dialog" aria-label={m.member_role_add_one()}>
          <Autocomplete inputValue={query} onInputChange={setQuery}>
            <AriaTextField
              className="field"
              maxLength={MAX_ROLE}
              autoFocus
              onKeyDown={(e) => {
                // Entrée sans choix dans la liste : ajoute le texte saisi.
                if (e.key === "Enter" && q && items.length === 0) pick(q);
              }}
            >
              <Label className="sr-only">{m.member_role_new()}</Label>
              <Input className="input" placeholder={m.member_role_placeholder()} />
            </AriaTextField>
            <ListBox
              className="listbox role-picker__list"
              aria-label={m.member_roles()}
              items={items}
              onAction={(id) => {
                const it = items.find((x) => x.id === id);
                if (it) pick(it.label);
              }}
              renderEmptyState={() => (
                <p className="muted role-picker__empty">{m.member_role_none()}</p>
              )}
            >
              {(it) => (
                <ListBoxItem id={it.id} className="listbox__item" textValue={it.label}>
                  {it.id === CREATE ? m.member_role_create({ role: it.label }) : it.label}
                </ListBoxItem>
              )}
            </ListBox>
          </Autocomplete>
        </Dialog>
      </Popover>
    </DialogTrigger>
  );
}
