import { useMemo, useState } from "react";
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
import { matches, Search } from "../../ui/Search";
import { FoldersDialog } from "./FoldersDialog";
import { ancestry, buildTree, membersUnder, pathOf } from "./folderTree";
import { MemberDialog } from "./MemberDialog";
import { PAGE } from "./MemberPicker";
import { MemberProfileDialog } from "./MemberProfile";
import { RolesDialog } from "./RolesDialog";
import { useFolders } from "./useFolders";
import { useMembers } from "./useMembers";

const ALL = "all";

export function MembersView() {
  const members = useMembers();
  const { list } = members;
  const folders = useFolders();
  const tree = useMemo(() => buildTree(folders.list), [folders.list]);
  const [filter, setFilter] = useState(ALL);
  const [q, setQ] = useState("");
  const [limit, setLimit] = useState(PAGE);
  // Un dossier supprimé (ici ou ailleurs) ramène à tous les membres.
  const folder = tree.byId.get(filter);
  const shown = useMemo(() => {
    const inFolder = folder ? membersUnder(tree, folder.id) : null;
    return list.filter((x) => (!inFolder || inFolder.has(x.id)) && matches(q, x));
  }, [list, folder, tree, q]);
  const pick = (id: string) => {
    setFilter(id);
    setLimit(PAGE);
  };
  // Une rangée de choix par niveau : la racine, puis les sous-dossiers du dossier choisi.
  const path = folder ? ancestry(tree, folder.id) : [];
  const levels = ["", ...path].filter((k) => (tree.children.get(k) ?? []).length > 0);
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

      {list.length > 0 && (
        <div className="members__tools">
          <Search
            label={m.search_members()}
            value={q}
            onChange={(v) => {
              setQ(v);
              setLimit(PAGE);
            }}
          />
          {levels.length > 0 && (
            <div className="folder-filter">
              {levels.map((key, depth) => {
                const parent = key ? tree.byId.get(key) : undefined;
                const all = parent ? parent.id : ALL;
                // La valeur de la rangée : le dossier choisi à ce niveau, sinon « Tout ».
                const value = path[depth] ?? all;
                return (
                  <RadioGroup
                    key={key || ALL}
                    className="field"
                    value={value}
                    onChange={pick}
                    orientation="horizontal"
                  >
                    <Label className="sr-only">
                      {parent ? pathOf(tree, parent.id) : m.folders_filter()}
                    </Label>
                    <div className="choices">
                      <Radio value={all} className="choice">
                        {parent
                          ? m.folders_all_in({
                              folder: parent.name,
                              count: membersUnder(tree, parent.id).size,
                            })
                          : m.folders_all({ count: list.length })}
                      </Radio>
                      {(tree.children.get(key) ?? []).map((f) => (
                        <Radio key={f.id} value={f.id} className="choice">
                          {f.name} ({membersUnder(tree, f.id).size})
                        </Radio>
                      ))}
                    </div>
                  </RadioGroup>
                );
              })}
            </div>
          )}
        </div>
      )}

      {list.length > 0 && shown.length === 0 && (
        <p className="muted">{q.trim() ? m.members_none_found() : m.folders_empty_folder()}</p>
      )}

      {shown.length > 0 && (
        <ul className="members">
          {shown.slice(0, limit).map((member) => (
            <MemberCard key={member.id} member={member} />
          ))}
        </ul>
      )}
      {shown.length > limit && (
        <Button variant="ghost" onPress={() => setLimit((n) => n + PAGE * 2)}>
          {m.search_more({ count: shown.length - limit })}
        </Button>
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
