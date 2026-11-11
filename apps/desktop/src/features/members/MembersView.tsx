import { Button as AriaButton, DialogTrigger } from "react-aria-components";
import type { Member } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Avatar } from "../../ui/Avatar";
import { Button } from "../../ui/Button";
import { MemberDialog } from "./MemberDialog";
import { useMembers } from "./useMembers";

export function MembersView() {
  const members = useMembers();
  const { list } = members;

  const addButton = (
    <DialogTrigger>
      <Button variant="primary">{m.members_add()}</Button>
      <MemberDialog />
    </DialogTrigger>
  );

  return (
    <section className="view">
      <header className="view__header">
        <div>
          <h1>{m.members_title()}</h1>
          <p className="view__lead">{m.members_lead()}</p>
        </div>
        {list.length > 0 && addButton}
      </header>

      {members.isSuccess && list.length === 0 && (
        <div className="empty">
          <h2>{m.members_empty_title()}</h2>
          <p>{m.members_empty_body()}</p>
          {addButton}
        </div>
      )}

      {list.length > 0 && (
        <ul className="members">
          {list.map((member) => (
            <MemberCard key={member.id} member={member} />
          ))}
        </ul>
      )}
    </section>
  );
}

function MemberCard({ member }: { member: Member }) {
  return (
    <li>
      <DialogTrigger>
        <AriaButton
          className="member"
          aria-label={`${m.member_edit()} ${member.name}`}
          style={{ "--member": member.color } as React.CSSProperties}
        >
          <Avatar name={member.name} color={member.color} src={member.avatar} size={48} />
          <span className="member__text">
            <span className="member__name">{member.name}</span>
            {member.pronouns && <span className="member__pronouns">{member.pronouns}</span>}
            {member.roles.length > 0 && (
              <span className="member__roles">{member.roles.join(" · ")}</span>
            )}
            {member.description && (
              <span className="member__description">{member.description}</span>
            )}
          </span>
        </AriaButton>
        <MemberDialog member={member} />
      </DialogTrigger>
    </li>
  );
}
