import { Checkbox, CheckboxGroup, Radio, RadioGroup } from "react-aria-components";
import type { Audience, Member, Privacy, Rule } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Avatar } from "../../ui/Avatar";
import { PeopleField } from "../messages/PeopleField";
import { friendName, useFriends } from "./useFriends";

type Flag = Exclude<keyof Privacy, "hidden">;

type Row = { key: Flag; label: () => string; hint: () => string };

/** Les réglages par catégorie, comme les pages de réglages de Discord. */
const CATEGORIES: { title: () => string; rows: Row[] }[] = [
  {
    title: m.privacy_cat_profile,
    rows: [
      { key: "members", label: m.friends_privacy_members, hint: m.friends_privacy_members_hint },
      { key: "avatars", label: m.friends_privacy_avatars, hint: m.friends_privacy_avatars_hint },
      {
        key: "pronouns",
        label: m.friends_privacy_pronouns,
        hint: m.friends_privacy_pronouns_hint,
      },
      {
        key: "descriptions",
        label: m.friends_privacy_descriptions,
        hint: m.friends_privacy_descriptions_hint,
      },
    ],
  },
  {
    title: m.privacy_cat_activity,
    rows: [{ key: "front", label: m.friends_privacy_front, hint: m.friends_privacy_front_hint }],
  },
  {
    title: m.privacy_cat_messages,
    rows: [
      { key: "messages", label: m.friends_privacy_messages, hint: m.friends_privacy_messages_hint },
    ],
  },
];

const LEVELS: { who: Audience; label: () => string }[] = [
  { who: "everyone", label: m.privacy_level_everyone },
  { who: "friends", label: m.privacy_level_friends },
  { who: "chosen", label: m.privacy_level_chosen },
  { who: "nobody", label: m.privacy_level_nobody },
];

/** Ces informations n'existent pas sans la liste des membres. */
const NEEDS_MEMBERS = new Set<Flag>(["avatars", "pronouns", "descriptions", "front"]);

/**
 * Qui voit quoi : pour chaque information, tout le monde, les amis, une liste
 * d'amis choisis ou personne ; puis les membres jamais montrés.
 */
export function PrivacyForm({
  value,
  onChange,
  members,
}: {
  value: Privacy;
  onChange: (next: Privacy) => void;
  members: Member[];
}) {
  const byId = new Map(members.map((x) => [x.id, x]));
  const noMembers = value.members.who === "nobody";
  return (
    <div className="privacy">
      <p className="muted">{m.privacy_everyone_note()}</p>
      {CATEGORIES.map((cat) => (
        <section key={cat.title()} className="privacy__cat">
          <h3 className="privacy__cat-title">{cat.title()}</h3>
          {cat.rows.map((row) => (
            <RuleRow
              key={row.key}
              row={row}
              rule={value[row.key]}
              // Écrire suppose d'être ami : « tout le monde » n'a pas de sens ici.
              levels={row.key === "messages" ? LEVELS.filter((l) => l.who !== "everyone") : LEVELS}
              isDisabled={noMembers && NEEDS_MEMBERS.has(row.key)}
              onChange={(rule) => onChange({ ...value, [row.key]: rule })}
            />
          ))}
        </section>
      ))}
      {!noMembers && (
        <section className="privacy__cat">
          <h3 className="privacy__cat-title">{m.friends_privacy_hidden()}</h3>
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
          <span className="field__hint">{m.friends_privacy_hidden_hint()}</span>
        </section>
      )}
    </div>
  );
}

function RuleRow({
  row,
  rule,
  levels,
  isDisabled,
  onChange,
}: {
  row: Row;
  rule: Rule;
  levels: typeof LEVELS;
  isDisabled: boolean;
  onChange: (rule: Rule) => void;
}) {
  const id = `privacy-${row.key}`;
  return (
    <div className="privacy__row" data-disabled={isDisabled || undefined}>
      <div className="privacy__text">
        <strong id={id}>{row.label()}</strong>
        <span className="field__hint">{isDisabled ? m.privacy_needs_members() : row.hint()}</span>
      </div>
      <RadioGroup
        className="seg privacy__seg"
        orientation="horizontal"
        aria-labelledby={id}
        isDisabled={isDisabled}
        value={rule.who}
        onChange={(who) => onChange({ who: who as Audience, friends: rule.friends ?? [] })}
      >
        {levels.map((l) => (
          <Radio key={l.who} value={l.who} className="seg__item">
            {l.label()}
          </Radio>
        ))}
      </RadioGroup>
      {rule.who === "chosen" && !isDisabled && (
        <ChosenFriends
          label={m.privacy_chosen_label({ what: row.label() })}
          value={rule.friends ?? []}
          onChange={(friends) => onChange({ who: "chosen", friends })}
        />
      )}
    </div>
  );
}

/** La liste d'amis qui ont accès à un réglage. */
function ChosenFriends({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string[];
  onChange: (ids: string[]) => void;
}) {
  const friends = useFriends();
  if (friends.list.length === 0) return <p className="muted">{m.privacy_chosen_none()}</p>;
  return (
    <CheckboxGroup
      className="picker privacy__friends"
      aria-label={label}
      value={value}
      onChange={onChange}
    >
      {friends.list.map((f) => (
        <Checkbox key={f.id} value={f.id} className="pick">
          <Avatar name={friendName(f)} src={f.avatar} size={28} />
          <span className="pick__name">{friendName(f)}</span>
        </Checkbox>
      ))}
    </CheckboxGroup>
  );
}
