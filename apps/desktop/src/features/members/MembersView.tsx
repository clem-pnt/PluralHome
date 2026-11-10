import { useQuery } from "@tanstack/react-query";
import { DialogTrigger } from "react-aria-components";
import { keys } from "../../app/query";
import { api, type Member } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { NewMemberDialog } from "./NewMemberDialog";

export function MembersView() {
  const members = useQuery({ queryKey: keys.members, queryFn: api.members.list });
  const list = members.data ?? [];

  const addButton = (
    <DialogTrigger>
      <Button variant="primary">{m.members_add()}</Button>
      <NewMemberDialog />
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
    <li className="member" style={{ "--member": member.color } as React.CSSProperties}>
      <span className="member__avatar" aria-hidden>
        {member.name.slice(0, 1).toLocaleUpperCase()}
      </span>
      <span className="member__text">
        <span className="member__name" data-selectable>
          {member.name}
        </span>
        {member.pronouns && <span className="member__pronouns">{member.pronouns}</span>}
      </span>
    </li>
  );
}
