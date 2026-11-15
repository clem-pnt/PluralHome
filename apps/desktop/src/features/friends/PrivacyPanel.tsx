import { useMutation, useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { Switch } from "react-aria-components";
import { keys, queryClient } from "../../app/query";
import { formatWhen } from "../../i18n/format";
import { api, type Privacy } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { useMembers } from "../members/useMembers";
import { FriendError } from "./FriendError";
import { PrivacyForm } from "./PrivacyForm";
import { refreshFriends } from "./useFriends";

/** Réglages de confidentialité : par défaut pour tous les amis, demandes, blocages. */
export function PrivacyPanel() {
  return (
    <>
      <OpenPanel />
      <DefaultPrivacyPanel />
      <BlocksPanel />
    </>
  );
}

function OpenPanel() {
  const settings = useQuery({ queryKey: keys.friendsSettings, queryFn: api.friends.settings });
  const set = useMutation({
    mutationFn: api.friends.setOpen,
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: keys.friendsSettings });
      void queryClient.invalidateQueries({ queryKey: keys.friendsInvites });
    },
  });
  return (
    <section className="panel">
      <h2 className="panel__title">{m.friends_open_title()}</h2>
      <div className="field">
        <Switch
          className="choice"
          isSelected={settings.data?.open ?? true}
          isDisabled={!settings.data || set.isPending}
          onChange={(open) => set.mutate(open)}
        >
          {m.friends_open()}
        </Switch>
        <span className="field__hint">{m.friends_open_hint()}</span>
      </div>
      <FriendError error={settings.error ?? set.error} />
    </section>
  );
}

function DefaultPrivacyPanel() {
  const members = useMembers();
  const current = useQuery({ queryKey: keys.friendsPrivacy, queryFn: api.friends.privacy });
  const [draft, setDraft] = useState<Privacy | null>(null);
  useEffect(() => {
    if (current.data) setDraft(current.data);
  }, [current.data]);
  const save = useMutation({
    mutationFn: api.friends.setPrivacy,
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: keys.friendsPrivacy });
      refreshFriends();
    },
  });
  const dirty = draft !== null && JSON.stringify(draft) !== JSON.stringify(current.data);

  return (
    <section className="panel">
      <h2 className="panel__title">{m.friends_privacy_title()}</h2>
      <p className="muted">{m.friends_privacy_body()}</p>
      {draft && <PrivacyForm value={draft} onChange={setDraft} members={members.list} />}
      <FriendError error={current.error ?? save.error} />
      <div className="panel__actions">
        <Button isDisabled={!dirty || save.isPending} onPress={() => draft && save.mutate(draft)}>
          {save.isPending ? m.working() : m.friends_save()}
        </Button>
        {save.isSuccess && !dirty && <span className="muted">{m.friends_saved()}</span>}
      </div>
    </section>
  );
}

function BlocksPanel() {
  const blocks = useQuery({ queryKey: keys.friendsBlocks, queryFn: api.friends.blocks });
  const unblock = useMutation({
    mutationFn: api.friends.unblock,
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: keys.friendsBlocks }),
  });
  const list = blocks.data ?? [];
  return (
    <section className="panel">
      <h2 className="panel__title">{m.friends_blocks_title()}</h2>
      <p className="muted">{m.friends_blocks_body()}</p>
      {list.length === 0 ? (
        <p className="muted">{m.friends_blocks_none()}</p>
      ) : (
        <ul className="friends-list">
          {list.map((b) => (
            <li key={b.id} className="friends-row">
              <div className="friends-row__text">
                <strong>{b.name ?? m.friends_unnamed()}</strong>
                {b.createdAt !== null && (
                  <span className="muted">
                    {m.friends_blocked_at({ when: formatWhen(b.createdAt) })}
                  </span>
                )}
              </div>
              <Button
                variant="ghost"
                isDisabled={unblock.isPending}
                onPress={() => unblock.mutate(b.id)}
              >
                {m.friends_unblock()}
              </Button>
            </li>
          ))}
        </ul>
      )}
      <FriendError error={blocks.error ?? unblock.error} />
    </section>
  );
}
