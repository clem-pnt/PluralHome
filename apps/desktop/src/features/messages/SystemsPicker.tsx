import { Button as AriaButton, Checkbox, Disclosure, DisclosurePanel } from "react-aria-components";
import { m } from "../../paraglide/messages.js";
import { Avatar } from "../../ui/Avatar";
import { Icon } from "../../ui/Icon";
import { MemberPicker, type Pickable } from "../members/MemberPicker";

/** Notre propre système, à côté de ceux des amis. */
export const SELF = "self";

export interface PickSystem {
  /** `SELF` ou l'identifiant de l'ami. */
  id: string;
  name: string;
  avatar: string | null;
  members: Pickable[];
}

/** Un système et certains de ses membres ; aucun membre : le système entier. */
export interface SystemPick {
  system: string;
  ids: string[];
}

/**
 * Liste de systèmes, chacun avec ses alters dans une liste déroulante, et la
 * possibilité de choisir le système entier. Un seul système à la fois : un
 * échange réunit notre système et, au plus, celui d'un ami.
 */
export function SystemsPicker({
  label,
  systems,
  value,
  onChange,
  wholeLabel = () => m.systems_whole(),
}: {
  label: string;
  systems: PickSystem[];
  value: SystemPick | null;
  onChange: (value: SystemPick | null) => void;
  wholeLabel?: (system: PickSystem) => string;
}) {
  return (
    <fieldset className="systems">
      <legend className="field__label">{label}</legend>
      {systems.map((s) => (
        <SystemRow
          key={s.id}
          system={s}
          pick={value?.system === s.id ? value.ids : null}
          onChange={(ids) => onChange(ids === null ? null : { system: s.id, ids })}
          wholeLabel={wholeLabel(s)}
          expanded={systems.length === 1}
        />
      ))}
    </fieldset>
  );
}

/**
 * Un système de la liste. `pick` : `null` s'il n'est pas choisi, sinon ses
 * membres choisis (aucun : le système entier).
 */
export function SystemRow({
  system: s,
  pick,
  onChange,
  wholeLabel = m.systems_whole(),
  expanded = false,
}: {
  system: PickSystem;
  pick: string[] | null;
  onChange: (pick: string[] | null) => void;
  wholeLabel?: string;
  expanded?: boolean;
}) {
  const on = pick !== null;
  const ids = pick ?? [];
  const whole = on && ids.length === 0;
  return (
    <Disclosure
      className="systems__item"
      data-selected={on || undefined}
      defaultExpanded={(on && ids.length > 0) || expanded}
    >
      <div className="systems__head">
        <Avatar name={s.name} src={s.avatar} size={32} />
        <span className="systems__name">
          <strong>{s.name}</strong>
          <span className="muted">
            {whole
              ? wholeLabel
              : ids.length > 0
                ? m.picker_selected({ count: ids.length })
                : m.group_member_count({ count: s.members.length })}
          </span>
        </span>
        <Checkbox className="choice" isSelected={whole} onChange={(v) => onChange(v ? [] : null)}>
          {wholeLabel}
        </Checkbox>
        <AriaButton
          slot="trigger"
          className="systems__toggle"
          aria-label={m.systems_members_of({ name: s.name })}
        >
          <Icon name="chevron-down" />
        </AriaButton>
      </div>
      <DisclosurePanel className="systems__panel">
        {s.members.length === 0 ? (
          <p className="muted">{m.systems_no_members()}</p>
        ) : (
          <MemberPicker
            label={m.systems_members_of({ name: s.name })}
            members={s.members}
            value={ids}
            onChange={(next) => onChange(next.length > 0 ? next : null)}
          />
        )}
      </DisclosurePanel>
    </Disclosure>
  );
}
