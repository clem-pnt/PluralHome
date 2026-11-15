import { Switch } from "react-aria-components";
import type { Member, Privacy } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { PeopleField } from "../messages/PeopleField";

type Flag = Exclude<keyof Privacy, "hidden">;

const FLAGS: { key: Flag; label: () => string; hint: () => string }[] = [
  { key: "members", label: m.friends_privacy_members, hint: m.friends_privacy_members_hint },
  { key: "avatars", label: m.friends_privacy_avatars, hint: m.friends_privacy_avatars_hint },
  { key: "pronouns", label: m.friends_privacy_pronouns, hint: m.friends_privacy_pronouns_hint },
  {
    key: "descriptions",
    label: m.friends_privacy_descriptions,
    hint: m.friends_privacy_descriptions_hint,
  },
  { key: "front", label: m.friends_privacy_front, hint: m.friends_privacy_front_hint },
  { key: "messages", label: m.friends_privacy_messages, hint: m.friends_privacy_messages_hint },
];

/** Ce qu'un ami voit de nous : un interrupteur par information, et les membres masqués. */
export function PrivacyForm({
  value,
  onChange,
  members,
  isDisabled,
}: {
  value: Privacy;
  onChange: (next: Privacy) => void;
  members: Member[];
  isDisabled?: boolean;
}) {
  const byId = new Map(members.map((x) => [x.id, x]));
  const needsMembers = new Set<Flag>(["avatars", "pronouns", "descriptions", "front"]);
  return (
    <div className="friends-privacy">
      {FLAGS.map((f) => (
        <div key={f.key} className="field">
          <Switch
            className="choice"
            isSelected={value[f.key]}
            isDisabled={Boolean(isDisabled) || (needsMembers.has(f.key) && !value.members)}
            onChange={(on) => onChange({ ...value, [f.key]: on })}
          >
            {f.label()}
          </Switch>
          <span className="field__hint">{f.hint()}</span>
        </div>
      ))}
      {value.members && (
        <PeopleField
          label={m.friends_privacy_hidden()}
          title={m.friends_privacy_hidden()}
          lead={m.friends_privacy_hidden_hint()}
          emptyLabel={m.friends_privacy_hidden_none()}
          members={members}
          byId={byId}
          value={value.hidden}
          onChange={(hidden) => onChange({ ...value, hidden })}
        />
      )}
    </div>
  );
}
