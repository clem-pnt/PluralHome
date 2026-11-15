import { useMutation, useQuery } from "@tanstack/react-query";
import { useState } from "react";
import {
  Button as AriaButton,
  Dialog,
  DialogTrigger,
  Disclosure,
  DisclosurePanel,
  Form,
  Heading,
  Modal,
  ModalOverlay,
  Switch,
} from "react-aria-components";
import { keys } from "../../app/query";
import { formatWhen } from "../../i18n/format";
import { api, type Friend, type Privacy } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Avatar } from "../../ui/Avatar";
import { Button } from "../../ui/Button";
import { Icon } from "../../ui/Icon";
import { TextField } from "../../ui/TextField";
import { useMembers } from "../members/useMembers";
import { ConfirmButton, FriendError } from "./FriendError";
import { PrivacyForm } from "./PrivacyForm";
import { friendName, refreshFriends } from "./useFriends";

/** Un système ami : ce qu'il partage avec nous, et nos réglages pour lui. */
export function FriendCard({ friend: f }: { friend: Friend }) {
  const byId = new Map(f.members.map((x) => [x.id, x]));
  const fronting = (f.front ?? []).flatMap((id) => {
    const x = byId.get(id);
    return x ? [x] : [];
  });
  const name = friendName(f);

  const unfriend = useMutation({ mutationFn: api.friends.unfriend, onSuccess: refreshFriends });
  const block = useMutation({ mutationFn: api.friends.block, onSuccess: refreshFriends });

  return (
    <li className="friend">
      <div className="friend__head">
        <Avatar name={name} src={f.avatar} size={48} />
        <div className="friend__text">
          <strong>{name}</strong>
          {f.nickname && f.name && <span className="muted">{f.name}</span>}
          <span className="muted">{m.friends_code({ code: f.code })}</span>
        </div>
        <div className="friend__tags">
          {!f.acceptsMessages && <span className="tag">{m.friends_no_messages()}</span>}
          {f.privacy && <span className="tag tag--accent">{m.friends_custom_privacy()}</span>}
        </div>
      </div>

      {f.cardAt === null ? (
        <p className="muted">{m.friends_card_pending()}</p>
      ) : (
        <>
          {f.front !== null && (
            <p className="friend__front">
              <span className="muted">{m.friends_front()} </span>
              {fronting.length > 0
                ? fronting.map((x) => x.name).join(", ")
                : m.friends_front_nobody()}
            </p>
          )}
          {f.members.length > 0 ? (
            <Disclosure className="friend__members">
              <AriaButton slot="trigger" className="friend__toggle">
                <Icon name="users" size={16} />
                {m.friends_members_count({ count: f.members.length })}
                <Icon name="chevron-down" size={16} />
              </AriaButton>
              <DisclosurePanel>
                <ul className="friend__list">
                  {f.members.map((x) => (
                    <li key={x.id} className="friend__member">
                      <span
                        className="friend__dot"
                        style={{ background: x.color || "var(--line-strong)" }}
                        aria-hidden
                      />
                      <Avatar name={x.name} src={x.avatar} size={28} />
                      <div className="friend__text">
                        <span>
                          {x.name}
                          {x.pronouns && <span className="muted"> · {x.pronouns}</span>}
                        </span>
                        {x.description && <span className="muted">{x.description}</span>}
                      </div>
                    </li>
                  ))}
                </ul>
              </DisclosurePanel>
            </Disclosure>
          ) : (
            <p className="muted">{m.friends_members_hidden()}</p>
          )}
          {f.cardAt !== null && (
            <span className="muted friend__updated">
              {m.friends_card_at({ when: formatWhen(f.cardAt) })}
            </span>
          )}
        </>
      )}

      <div className="row-actions">
        <FriendSettings friend={f} />
        <ConfirmButton
          label={m.friends_unfriend()}
          title={m.friends_unfriend_title({ name })}
          body={m.friends_unfriend_body()}
          confirm={m.friends_unfriend()}
          pending={unfriend.isPending}
          error={unfriend.error}
          onConfirm={(close) => unfriend.mutate(f.id, { onSuccess: close })}
        />
        <ConfirmButton
          label={m.friends_block()}
          variant="danger"
          title={m.friends_block_title({ name })}
          body={m.friends_block_body()}
          confirm={m.friends_block()}
          pending={block.isPending}
          error={block.error}
          onConfirm={(close) => block.mutate(f.id, { onSuccess: close })}
        />
      </div>
    </li>
  );
}

/** Surnom et confidentialité propres à cet ami. */
function FriendSettings({ friend: f }: { friend: Friend }) {
  return (
    <DialogTrigger>
      <Button variant="ghost">{m.friends_settings()}</Button>
      <ModalOverlay className="overlay" isDismissable>
        <Modal className="modal modal--wide">
          <Dialog className="dialog">
            {({ close }) => <FriendSettingsForm friend={f} close={close} />}
          </Dialog>
        </Modal>
      </ModalOverlay>
    </DialogTrigger>
  );
}

function FriendSettingsForm({ friend: f, close }: { friend: Friend; close: () => void }) {
  const members = useMembers();
  const defaults = useQuery({ queryKey: keys.friendsPrivacy, queryFn: api.friends.privacy });
  const [nickname, setNickname] = useState(f.nickname ?? "");
  const [custom, setCustom] = useState(f.privacy !== null);
  const [privacy, setPrivacy] = useState<Privacy | null>(f.privacy);
  const shown = privacy ?? defaults.data ?? null;

  const save = useMutation({
    mutationFn: () => api.friends.setPref(f.id, nickname.trim() || null, custom ? shown : null),
    onSuccess: () => {
      refreshFriends();
      close();
    },
  });

  return (
    <Form
      className="dialog__form"
      onSubmit={(e) => {
        e.preventDefault();
        save.mutate();
      }}
    >
      <Heading slot="title">{m.friends_settings_title({ name: friendName(f) })}</Heading>
      <TextField
        label={m.friends_nickname()}
        hint={m.friends_nickname_hint()}
        maxLength={100}
        value={nickname}
        onChange={setNickname}
      />
      <div className="field">
        <Switch className="choice" isSelected={custom} onChange={setCustom}>
          {m.friends_privacy_custom()}
        </Switch>
        <span className="field__hint">{m.friends_privacy_custom_hint()}</span>
      </div>
      {custom && shown && (
        <PrivacyForm value={shown} onChange={setPrivacy} members={members.list} />
      )}
      <FriendError error={save.error} />
      <div className="dialog__actions">
        <Button variant="ghost" onPress={close}>
          {m.cancel()}
        </Button>
        <Button type="submit" isDisabled={save.isPending || (custom && !shown)}>
          {save.isPending ? m.working() : m.friends_save()}
        </Button>
      </div>
    </Form>
  );
}
