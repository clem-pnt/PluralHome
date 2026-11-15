import { useMutation, useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { Dialog, DialogTrigger, Heading, Modal, ModalOverlay, Switch } from "react-aria-components";
import { keys, queryClient } from "../../app/query";
import { formatWhen } from "../../i18n/format";
import { api, type Invite } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { Select } from "../../ui/Select";
import { FriendError } from "./FriendError";
import { qrDataUrl } from "./qr";

const HOUR = 3_600_000;
const DURATIONS: Record<string, number | null> = {
  day: 24 * HOUR,
  week: 7 * 24 * HOUR,
  never: null,
};

/** Liens d'invitation : on en crée un, on le montre en QR ou on le copie. */
export function InvitePanel() {
  const invites = useQuery({ queryKey: keys.friendsInvites, queryFn: api.friends.invites });
  const [duration, setDuration] = useState("week");
  const [once, setOnce] = useState(true);
  const [fresh, setFresh] = useState<Invite | null>(null);
  const create = useMutation({
    mutationFn: () => api.friends.createInvite(DURATIONS[duration] ?? null, once),
    onSuccess: (inv) => {
      setFresh(inv);
      void queryClient.invalidateQueries({ queryKey: keys.friendsInvites });
    },
  });
  const revoke = useMutation({
    mutationFn: api.friends.revokeInvite,
    onSuccess: (_, id) => {
      if (fresh?.id === id) setFresh(null);
      void queryClient.invalidateQueries({ queryKey: keys.friendsInvites });
    },
  });

  const list = invites.data ?? [];

  return (
    <div className="panel">
      <h2 className="panel__title">{m.friends_invite_title()}</h2>
      <p className="muted">{m.friends_invite_body()}</p>
      <div className="friends-invite__form">
        <Select
          label={m.friends_invite_duration()}
          options={[
            { id: "day", label: m.friends_invite_day() },
            { id: "week", label: m.friends_invite_week() },
            { id: "never", label: m.friends_invite_never() },
          ]}
          value={duration}
          onChange={setDuration}
        />
        <Switch className="choice" isSelected={once} onChange={setOnce}>
          {m.friends_invite_once()}
        </Switch>
      </div>
      <div>
        <Button isDisabled={create.isPending} onPress={() => create.mutate()}>
          {create.isPending ? m.working() : m.friends_invite_create()}
        </Button>
      </div>
      <FriendError error={create.error} />

      {fresh?.link && <InviteShare link={fresh.link} />}

      {list.length > 0 && (
        <>
          <h3 className="friends__sub">{m.friends_invite_active()}</h3>
          <ul className="friends-list">
            {list.map((inv) => (
              <li key={inv.id} className="friends-row">
                <div className="friends-row__text">
                  <span>
                    {inv.once ? m.friends_invite_kind_once() : m.friends_invite_kind_many()}
                  </span>
                  <span className="muted">
                    {inv.expiresAt
                      ? m.friends_invite_until({ when: formatWhen(inv.expiresAt) })
                      : m.friends_invite_forever()}
                    {inv.createdAt !== null &&
                      ` · ${m.friends_invite_created({ when: formatWhen(inv.createdAt) })}`}
                  </span>
                </div>
                <div className="row-actions">
                  {inv.link ? (
                    <ShowInvite link={inv.link} />
                  ) : (
                    <span className="muted">{m.friends_invite_other_device()}</span>
                  )}
                  <Button
                    variant="ghost"
                    isDisabled={revoke.isPending}
                    onPress={() => revoke.mutate(inv.id)}
                  >
                    {m.friends_invite_revoke()}
                  </Button>
                </div>
              </li>
            ))}
          </ul>
        </>
      )}
      <FriendError error={revoke.error} />
    </div>
  );
}

function ShowInvite({ link }: { link: string }) {
  return (
    <DialogTrigger>
      <Button variant="ghost">{m.friends_invite_show()}</Button>
      <ModalOverlay className="overlay" isDismissable>
        <Modal className="modal">
          <Dialog className="dialog">
            {({ close }) => (
              <>
                <Heading slot="title">{m.friends_invite_title()}</Heading>
                <InviteShare link={link} />
                <div className="dialog__actions">
                  <Button variant="ghost" onPress={close}>
                    {m.close()}
                  </Button>
                </div>
              </>
            )}
          </Dialog>
        </Modal>
      </ModalOverlay>
    </DialogTrigger>
  );
}

/** QR code du lien, et le lien à copier. */
function InviteShare({ link }: { link: string }) {
  const [qr, setQr] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    let live = true;
    setCopied(false);
    qrDataUrl(link).then(
      (url) => live && setQr(url),
      () => live && setQr(null),
    );
    return () => {
      live = false;
    };
  }, [link]);

  const copy = () => {
    void navigator.clipboard?.writeText(link).then(
      () => setCopied(true),
      () => setCopied(false),
    );
  };

  return (
    <div className="friends-share">
      {qr && <img className="friends-share__qr" src={qr} alt={m.friends_invite_qr_alt()} />}
      <div className="friends-share__side">
        <p className="muted">{m.friends_invite_share_hint()}</p>
        <input
          className="input friends-share__link"
          readOnly
          value={link}
          aria-label={m.friends_add_link()}
          onFocus={(e) => e.currentTarget.select()}
        />
        <div>
          <Button variant="ghost" onPress={copy}>
            {copied ? m.friends_invite_copied() : m.friends_invite_copy()}
          </Button>
        </div>
        <p className="note">{m.friends_invite_warning()}</p>
      </div>
    </div>
  );
}
