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
      {systems.map((s) => {
        const on = value?.system === s.id;
        const ids = on ? value.ids : [];
        const whole = on && ids.length === 0;
        return (
          <Disclosure
            key={s.id}
            className="systems__item"
            data-selected={on || undefined}
            defaultExpanded={(on && ids.length > 0) || systems.length === 1}
          >
            <div className="systems__head">
              <Avatar name={s.name} src={s.avatar} size={32} />
              <span className="systems__name">
                <strong>{s.name}</strong>
                <span className="muted">
                  {whole
                    ? wholeLabel(s)
                    : ids.length > 0
                      ? m.picker_selected({ count: ids.length })
                      : m.group_member_count({ count: s.members.length })}
                </span>
              </span>
              <Checkbox
                className="choice"
                isSelected={whole}
                onChange={(v) => onChange(v ? { system: s.id, ids: [] } : null)}
              >
                {wholeLabel(s)}
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
                  onChange={(next) =>
                    onChange(next.length > 0 ? { system: s.id, ids: next } : null)
                  }
                />
              )}
            </DisclosurePanel>
          </Disclosure>
        );
      })}
    </fieldset>
  );
}
