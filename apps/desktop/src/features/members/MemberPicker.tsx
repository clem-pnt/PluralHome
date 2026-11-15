import { useMemo, useState } from "react";
import { Checkbox, CheckboxGroup } from "react-aria-components";
import { m } from "../../paraglide/messages.js";
import { Avatar } from "../../ui/Avatar";
import { Button } from "../../ui/Button";
import { matches, Search } from "../../ui/Search";

/** Au-delà, on n'affiche que les premiers résultats : la recherche fait le reste. */
export const PAGE = 60;

/** Ce qu'il faut pour proposer quelqu'un : nos membres comme ceux d'un ami. */
export type Pickable = { id: string; name: string; avatar?: string | null | undefined };

/**
 * Choix de plusieurs membres avec recherche. Pensé pour les systèmes de plusieurs
 * centaines de membres : seuls les premiers résultats sont rendus, et la sélection
 * survit au filtrage.
 */
export function MemberPicker({
  label,
  members,
  value,
  onChange,
  autoFocus,
  single,
}: {
  label: string;
  members: Pickable[];
  value: string[];
  onChange: (ids: string[]) => void;
  autoFocus?: boolean;
  /** Un seul membre à la fois : en cocher un autre remplace le précédent. */
  single?: boolean;
}) {
  const [q, setQ] = useState("");
  const [limit, setLimit] = useState(PAGE);
  const found = useMemo(() => members.filter((x) => matches(q, x)), [members, q]);
  const shown = found.slice(0, limit);

  return (
    <div className="field">
      {members.length > 8 && (
        <Search
          label={m.search_members()}
          value={q}
          onChange={(v) => {
            setQ(v);
            setLimit(PAGE);
          }}
          {...(autoFocus && { autoFocus })}
        />
      )}
      {!single && value.length > 0 && (
        <p className="muted picker__count">{m.picker_selected({ count: value.length })}</p>
      )}
      <CheckboxGroup
        className="picker"
        aria-label={label}
        value={value}
        // Le groupe garde aussi les membres cochés que la recherche masque.
        onChange={(ids) => {
          if (!single) return onChange([...new Set(ids)]);
          const added = ids.find((id) => !value.includes(id));
          onChange(added ? [added] : []);
        }}
      >
        {shown.map((x) => (
          <Checkbox key={x.id} value={x.id} className="pick">
            <Avatar name={x.name} src={x.avatar} size={32} />
            <span className="pick__name">{x.name}</span>
          </Checkbox>
        ))}
      </CheckboxGroup>
      {found.length === 0 && <p className="muted">{m.search_none()}</p>}
      {found.length > limit && (
        <Button variant="ghost" onPress={() => setLimit((n) => n + PAGE * 2)}>
          {m.search_more({ count: found.length - limit })}
        </Button>
      )}
    </div>
  );
}
