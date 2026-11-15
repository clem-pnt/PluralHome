import { useState } from "react";
import { Dialog, Heading, Modal, ModalOverlay } from "react-aria-components";
import { formatWhen } from "../../i18n/format";
import type { Member, Message } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Avatar } from "../../ui/Avatar";
import { Button } from "../../ui/Button";
import { IconButton } from "../../ui/IconButton";
import { EditMessageForm } from "./EditMessageDialog";
import { names, readersOf } from "./people";
import { useDeleteMessage, useMarkReadBy, usePinMessage } from "./useMessages";

export function MessageCard({
  message,
  byId,
  fronters = [],
  members = [],
  actions = true,
}: {
  message: Message;
  byId: Map<string, Member>;
  /** Membres au front ou en co-front, pour mettre en avant ce qui les concerne. */
  fronters?: string[];
  members?: Member[];
  actions?: boolean;
}) {
  const { authorIds, recipientIds, readBy } = message;
  const broadcast = recipientIds.length === 0;
  const fromFront = fronters.filter((id) => authorIds.includes(id));
  const toFront = broadcast ? [] : fronters.filter((id) => recipientIds.includes(id));

  const author = byId.get(authorIds[0] ?? "");
  const from = names(authorIds, byId) || m.messages_someone();
  const to = broadcast ? m.messages_everyone_route() : names(recipientIds, byId);
  const created = message.createdAt ?? 0;

  return (
    <article
      className="message"
      data-pinned={message.pinned || undefined}
      data-from-front={fromFront.length > 0 || undefined}
      data-to-front={toFront.length > 0 || undefined}
    >
      {author ? (
        <Avatar name={author.name} src={author.avatar} size={36} />
      ) : (
        <span
          className="avatar avatar--blurry"
          style={{ "--size": "36px" } as React.CSSProperties}
          aria-hidden
        />
      )}
      <div className="message__content">
        <header className="message__meta">
          <span>{m.messages_route({ from, to })}</span>
          <time className="muted" dateTime={new Date(created).toISOString()}>
            {formatWhen(created)}
          </time>
          {message.editedAt != null && <span className="muted">· {m.messages_edited()}</span>}
          {message.pinned && <span className="tag">{m.messages_pinned()}</span>}
          {fromFront.length > 0 && (
            <span className="tag tag--accent">
              {m.messages_badge_from({ names: names(fromFront, byId) })}
            </span>
          )}
          {toFront.length > 0 && (
            <span className="tag tag--accent">
              {m.messages_badge_to({ names: names(toFront, byId) })}
            </span>
          )}
        </header>
        <p className="message__body" data-selectable>
          {message.body}
        </p>
        {readBy.length > 0 && (
          <p className="message__read muted">
            {m.messages_read_by({ names: names(readBy, byId, 4) })}
          </p>
        )}
        {actions && (
          <MessageActions message={message} byId={byId} members={members} fronters={fronters} />
        )}
      </div>
    </article>
  );
}

/** Lu, épingler, modifier, supprimer : sous un message du tableau ou du fil. */
export function MessageActions({
  message,
  byId,
  members,
  fronters,
}: {
  message: Message;
  byId: Map<string, Member>;
  members: Member[];
  fronters: string[];
}) {
  const pin = usePinMessage();
  const remove = useDeleteMessage();
  const markRead = useMarkReadBy();
  const [dialog, setDialog] = useState<"edit" | "delete" | null>(null);
  const { authorIds, recipientIds } = message;
  const broadcast = recipientIds.length === 0;
  const fromFront = fronters.some((id) => authorIds.includes(id));
  const toFront = !broadcast && fronters.some((id) => recipientIds.includes(id));
  const readers = readersOf(message, fronters);
  // Un message reçu se lit et s'épingle, mais ne se modifie ni ne se supprime.
  const received = toFront && !fromFront;

  return (
    <div className="message__actions">
      {readers.length > 0 && (
        <IconButton
          icon="check"
          tone="accent"
          label={m.messages_mark_read()}
          isDisabled={markRead.isPending}
          onPress={() => markRead.mutate({ id: message.id, members: readers })}
        />
      )}
      <IconButton
        icon="pin"
        label={message.pinned ? m.messages_unpin() : m.messages_pin()}
        pressed={message.pinned}
        isDisabled={pin.isPending}
        onPress={() => pin.mutate({ id: message.id, pinned: !message.pinned })}
      />
      {!received && (
        <>
          <IconButton icon="edit" label={m.messages_edit()} onPress={() => setDialog("edit")} />
          <IconButton
            icon="trash"
            tone="danger"
            label={m.messages_delete()}
            onPress={() => setDialog("delete")}
          />
        </>
      )}
      <ModalOverlay
        className="overlay"
        isDismissable
        isOpen={dialog !== null}
        onOpenChange={(open) => !open && setDialog(null)}
      >
        <Modal className={dialog === "edit" ? "modal modal--wide" : "modal"}>
          <Dialog className="dialog" role={dialog === "delete" ? "alertdialog" : "dialog"}>
            {({ close }) =>
              dialog === "edit" ? (
                <EditMessageForm message={message} members={members} byId={byId} onDone={close} />
              ) : (
                <>
                  <Heading slot="title">{m.messages_delete_title()}</Heading>
                  <p className="muted">{m.messages_delete_body()}</p>
                  <div className="dialog__actions">
                    <Button variant="ghost" slot="close">
                      {m.cancel()}
                    </Button>
                    <Button
                      variant="danger"
                      autoFocus
                      onPress={() => remove.mutate(message.id, { onSuccess: close })}
                    >
                      {m.messages_delete()}
                    </Button>
                  </div>
                </>
              )
            }
          </Dialog>
        </Modal>
      </ModalOverlay>
    </div>
  );
}
