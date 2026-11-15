import { useState } from "react";
import {
  Button as AriaButton,
  DialogTrigger,
  Label,
  Radio,
  RadioGroup,
} from "react-aria-components";
import type { Member } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Avatar } from "../../ui/Avatar";
import { Button } from "../../ui/Button";
import { FoldersDialog } from "./FoldersDialog";
import { MemberDialog } from "./MemberDialog";
import { MemberProfileDialog } from "./MemberProfile";
import { RolesDialog } from "./RolesDialog";
import { useFolders } from "./useFolders";
import { useMembers } from "./useMembers";

const ALL = "all";

export function MembersView() {
  const members = useMembers();
  const { list } = members;
  const folders = useFolders();
  const [filter, setFilter] = useState(ALL);
  // Un dossier supprimé (ici ou ailleurs) ramène à tous les membres.
  const folder = folders.list.find((f) => f.id === filter);
  const shown = folder ? list.filter((x) => folder.members.includes(x.id)) : list;
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
        {list.length > 0 && (
          <div className="panel__actions">
            <DialogTrigger>
              <Button variant="ghost">{m.folders_manage()}</Button>
              <FoldersDialog />
            </DialogTrigger>
            <DialogTrigger>
              <Button variant="ghost">{m.roles_manage()}</Button>
              <RolesDialog />
            </DialogTrigger>
            {addButton}
          </div>
        )}
      </header>

      {members.isSuccess && list.length === 0 && (
        <div className="empty">
          <h2>{m.members_empty_title()}</h2>
          <p>{m.members_empty_body()}</p>
          {addButton}
        </div>
      )}

      {list.length > 0 && folders.list.length > 0 && (
        <RadioGroup
          className="field folders__filter"
          value={folder ? folder.id : ALL}
          onChange={setFilter}
          orientation="horizontal"
        >
          <Label className="sr-only">{m.folders_filter()}</Label>
          <div className="choices">
            <Radio value={ALL} className="choice">
              {m.folders_all({ count: list.length })}
            </Radio>
            {folders.list.map((f) => (
              <Radio key={f.id} value={f.id} className="choice">
                {f.name} ({f.members.length})
              </Radio>
            ))}
          </div>
        </RadioGroup>
      )}

      {folder && shown.length === 0 && <p className="muted">{m.folders_empty_folder()}</p>}

      {shown.length > 0 && (
        <ul className="members">
          {shown.map((member) => (
            <MemberCard key={member.id} member={member} />
          ))}
        </ul>
      )}
    </section>
  );
}

function MemberCard({ member }: { member: Member }) {
  const main = member.roles[0];
  return (
    <li>
      <DialogTrigger>
        <AriaButton className="member" aria-label={`${m.member_profile_open()} ${member.name}`}>
          <Avatar name={member.name} src={member.avatar} size={48} />
          <span className="member__text">
            <span className="member__name">{member.name}</span>
            {member.pronouns && <span className="member__pronouns">{member.pronouns}</span>}
            {main && <span className="member__roles">{main}</span>}
          </span>
        </AriaButton>
        <MemberProfileDialog member={member} />
      </DialogTrigger>
    </li>
  );
}
