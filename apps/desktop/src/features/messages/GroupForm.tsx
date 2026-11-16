import { useState } from "react";
import { Form, Heading } from "react-aria-components";
import { errorMessage } from "../../i18n/errors";
import type { AvatarEdit, ChatGroup, Member } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { TextField } from "../../ui/TextField";
import { AvatarInput } from "../members/AvatarInput";
import { useFriendSystems, useSelfSystem } from "./NewConversation";
import { SystemRow } from "./SystemsPicker";
import { useDeleteGroup, useSaveGroup } from "./useGroups";

const MAX_NAME = 100;

/** Discussion privée (sans nom) à transformer en groupe, avec ses messages. */
export interface GroupSeed {
  name: string;
  memberIds: string[];
  friendId?: string;
  /** Membres de l'ami ; vide : tout son système. */
  remoteRefs: string[];
  messageIds: string[];
}

/**
 * Création ou modification d'un groupe : nom, photo, membres. Avec `seed`, une
 * discussion existante devient un groupe et garde son historique.
 */
export function GroupForm({
  group,
  seed,
  members,
  onDone,
  onCancel,
  onDeleted,
}: {
  group: ChatGroup | null;
  seed?: GroupSeed;
  members: Member[];
  onDone: (group: ChatGroup) => void;
  onCancel: () => void;
  onDeleted?: () => void;
}) {
  const save = useSaveGroup();
  const remove = useDeleteGroup();
  const [name, setName] = useState(group?.name ?? seed?.name ?? "");
  const self = useSelfSystem(members);
  // Notre côté : `null` personne, `[]` tout le système, sinon certains membres.
  const [ids, setIds] = useState<string[] | null>(() => {
    const start = group?.memberIds ?? seed?.memberIds;
    if (!start) return null;
    if (start.length === 0) return [];
    const all = new Set(members.map((x) => x.id));
    return start.length === all.size && start.every((id) => all.has(id)) ? [] : start;
  });
  const [avatar, setAvatar] = useState<string | null>(group?.avatar ?? null);
  const [avatarEdit, setAvatarEdit] = useState<AvatarEdit>({ kind: "keep" });
  const [confirm, setConfirm] = useState(false);
  const allFriends = useFriendSystems();
  // Le premier système ami d'un groupe est fixé à sa création (celui d'une
  // discussion reprise, par ses messages) ; d'autres peuvent s'y ajouter.
  // Un groupe resté entre nous ne se partage plus.
  const fixedFriend = group ? group.friendId : seed ? (seed.friendId ?? null) : undefined;
  const friends = fixedFriend === null ? [] : allFriends;
  // Systèmes amis choisis, dans l'ordre : membres retenus (vide : tout le système).
  const [picks, setPicks] = useState<[string, string[]][]>(() => {
    if (group?.friendId)
      return [
        [group.friendId, group.remoteMembers.map((r) => r.ref)],
        ...group.peers.flatMap((p): [string, string[]][] =>
          p.friendId ? [[p.friendId, p.members.map((r) => r.ref)]] : [],
        ),
      ];
    return seed?.friendId ? [[seed.friendId, seed.remoteRefs]] : [];
  });
  const primary = fixedFriend ?? picks[0]?.[0] ?? null;
  const pickOf = (id: string) => picks.find(([f]) => f === id)?.[1] ?? null;
  // Systèmes connus seulement par un autre membre du groupe.
  const far = (group?.peers ?? []).filter((p) => !p.friendId && p.name);
  const shared = picks.length > 0;
  const valid = name.trim().length > 0 && ids !== null && (ids.length > 0 || members.length > 0);
  // Tout notre système : un groupe partagé le note sans liste (les nouveaux
  // membres en feront partie), un groupe local liste chacun.
  const memberIds = ids === null ? [] : ids.length > 0 || shared ? ids : members.map((x) => x.id);

  if (confirm && group) {
    return (
      <>
        <Heading slot="title">{m.group_delete_title({ name: group.name })}</Heading>
        <p className="muted">{m.group_delete_body()}</p>
        {remove.isError && (
          <p className="alert" role="alert">
            {errorMessage(remove.error)}
          </p>
        )}
        <div className="dialog__actions">
          <Button variant="ghost" onPress={() => setConfirm(false)}>
            {m.cancel()}
          </Button>
          <Button
            variant="danger"
            autoFocus
            isDisabled={remove.isPending}
            onPress={() => remove.mutate(group.id, { onSuccess: () => onDeleted?.() })}
          >
            {m.group_delete()}
          </Button>
        </div>
      </>
    );
  }

  return (
    <Form
      className="dialog"
      style={{ padding: 0 }}
      onSubmit={(e) => {
        e.preventDefault();
        if (!valid) return;
        save.mutate(
          {
            id: group?.id ?? null,
            input: {
              name: name.trim(),
              memberIds,
              avatar: avatarEdit,
              friendId: primary,
              remoteMemberRefs: primary ? (pickOf(primary) ?? []) : [],
              peers: picks
                .filter(([f]) => f !== primary)
                .map(([friendId, memberRefs]) => ({ friendId, memberRefs })),
            },
            messageIds: seed?.messageIds,
          },
          { onSuccess: onDone },
        );
      }}
    >
      <Heading slot="title">
        {group
          ? m.group_edit_title({ name: group.name })
          : seed
            ? m.group_from_title()
            : m.group_new()}
      </Heading>
      {seed && <p className="muted">{m.group_from_hint()}</p>}
      <AvatarInput
        name={name}
        preview={avatar}
        onChange={(edit, preview) => {
          setAvatarEdit(edit);
          setAvatar(preview);
        }}
      />
      <TextField
        label={m.group_name()}
        autoFocus
        isRequired
        value={name}
        onChange={setName}
        maxLength={MAX_NAME}
      />
      <fieldset className="systems">
        <legend className="field__label">{m.group_members()}</legend>
        {friends.length > 0 && !fixedFriend && (
          <p className="field__hint">{m.group_members_hint()}</p>
        )}
        <SystemRow system={self} pick={ids} onChange={setIds} expanded={friends.length === 0} />
        {friends.map((f) => (
          <SystemRow
            key={f.id}
            system={f}
            pick={pickOf(f.id)}
            onChange={(next) => {
              // Le premier ami d'un groupe partagé y reste : on ne peut que
              // changer ses membres.
              const value = next === null && f.id === fixedFriend ? [] : next;
              setPicks((cur) => {
                const i = cur.findIndex(([id]) => id === f.id);
                if (value === null) return cur.filter(([id]) => id !== f.id);
                if (i < 0) return [...cur, [f.id, value]];
                return cur.map((p, j): [string, string[]] => (j === i ? [f.id, value] : p));
              });
            }}
          />
        ))}
        {far.length > 0 && (
          <p className="field__hint">
            {m.group_far_systems({ names: far.map((p) => p.name).join(", ") })}
          </p>
        )}
      </fieldset>
      {picks.length > 1 && <p className="field__hint">{m.group_many_systems_hint()}</p>}
      {save.isError && (
        <p className="alert" role="alert">
          {errorMessage(save.error)}
        </p>
      )}
      <div className="dialog__actions">
        {group && (
          <Button variant="ghost" onPress={() => setConfirm(true)}>
            {m.group_delete()}
          </Button>
        )}
        <span style={{ flex: 1 }} />
        <Button variant="ghost" onPress={onCancel}>
          {m.cancel()}
        </Button>
        <Button type="submit" isDisabled={!valid || save.isPending}>
          {group || seed ? m.messages_save() : m.group_create()}
        </Button>
      </div>
    </Form>
  );
}
