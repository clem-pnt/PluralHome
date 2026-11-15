import { useState } from "react";
import { Dialog, Heading, Modal, ModalOverlay } from "react-aria-components";
import type { Member } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Avatar } from "../../ui/Avatar";
import { Button } from "../../ui/Button";
import { buildTree, pathOf } from "./folderTree";
import { MemberForm } from "./MemberDialog";
import { foldersOf, useFolders } from "./useFolders";
import { useMembers } from "./useMembers";

/** Fiche complète d'un membre, en lecture ; « Modifier » ouvre le formulaire. */
export function MemberProfileDialog({ member }: { member: Member }) {
  const [editing, setEditing] = useState(false);
  return (
    <ModalOverlay className="overlay" isDismissable>
      <Modal className="modal modal--wide">
        <Dialog className="dialog" aria-label={member.name}>
          {({ close }) =>
            editing ? (
              <MemberForm
                member={member}
                onDone={() => setEditing(false)}
                onCancel={() => setEditing(false)}
                onClose={close}
              />
            ) : (
              <Profile id={member.id} fallback={member} onEdit={() => setEditing(true)} />
            )
          }
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}

function Profile({ id, fallback, onEdit }: { id: string; fallback: Member; onEdit: () => void }) {
  // Toujours la version à jour (après modification ou synchro).
  const member = useMembers().byId.get(id) ?? fallback;
  const all = useFolders().list;
  const tree = buildTree(all);
  const folders = foldersOf(all, id);
  const [main, ...others] = member.roles;

  return (
    <>
      <header className="profile__head">
        <Avatar name={member.name} src={member.avatar} size={72} />
        <div className="profile__id">
          <Heading slot="title">{member.name}</Heading>
          {member.pronouns && <span className="muted">{member.pronouns}</span>}
          {main && <span className="member__roles">{main}</span>}
        </div>
      </header>

      {member.roles.length > 0 && (
        <section className="profile__section">
          <h3 className="field__label">{m.member_roles()}</h3>
          <ul className="chips">
            {main && (
              <li className="chip chip--main chip--read">
                {main}
                {others.length > 0 && <small className="chip__tag">{m.member_role_main()}</small>}
              </li>
            )}
            {others.map((r) => (
              <li key={r} className="chip chip--read">
                {r}
              </li>
            ))}
          </ul>
        </section>
      )}

      {folders.length > 0 && (
        <section className="profile__section">
          <h3 className="field__label">{m.member_folders()}</h3>
          <ul className="chips">
            {folders.map((f) => (
              <li key={f.id} className="chip chip--read chip--folder">
                {pathOf(tree, f.id)}
              </li>
            ))}
          </ul>
        </section>
      )}

      <section className="profile__section">
        <h3 className="field__label">{m.member_description()}</h3>
        {member.description ? (
          <p className="profile__description">{member.description}</p>
        ) : (
          <p className="muted">{m.member_no_description()}</p>
        )}
      </section>

      <div className="dialog__actions">
        <Button variant="ghost" slot="close">
          {m.close()}
        </Button>
        <Button variant="primary" onPress={onEdit}>
          {m.member_edit()}
        </Button>
      </div>
    </>
  );
}
