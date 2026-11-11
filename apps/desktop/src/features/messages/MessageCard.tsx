import { Dialog, DialogTrigger, Heading, Modal, ModalOverlay } from "react-aria-components";
import { formatWhen } from "../../i18n/format";
import type { Member, Message } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Avatar } from "../../ui/Avatar";
import { Button } from "../../ui/Button";
import { useDeleteMessage, usePinMessage } from "./useMessages";

export function MessageCard({
  message,
  byId,
  actions = true,
}: {
  message: Message;
  byId: Map<string, Member>;
  actions?: boolean;
}) {
  const author = message.authorId ? byId.get(message.authorId) : undefined;
  const recipient = message.recipientId ? byId.get(message.recipientId) : undefined;
  const pin = usePinMessage();
  const remove = useDeleteMessage();

  return (
    <article className="message" data-pinned={message.pinned || undefined}>
      {author ? (
        <Avatar name={author.name} color={author.color} src={author.avatar} size={36} />
      ) : (
        <span
          className="avatar avatar--blurry"
          style={{ "--size": "36px" } as React.CSSProperties}
          aria-hidden
        />
      )}
      <div className="message__content">
        <header className="message__meta">
          <span>
            {m.messages_route({
              from: author?.name ?? m.messages_someone(),
              to: recipient?.name ?? m.messages_everyone_route(),
            })}
          </span>
          <time className="muted" dateTime={new Date(message.createdAt ?? 0).toISOString()}>
            {formatWhen(message.createdAt ?? 0)}
          </time>
          {message.pinned && <span className="tag">{m.messages_pinned()}</span>}
        </header>
        <p className="message__body" data-selectable>
          {message.body}
        </p>
        {actions && (
          <div className="message__actions">
            <Button
              variant="ghost"
              isDisabled={pin.isPending}
              onPress={() => pin.mutate({ id: message.id, pinned: !message.pinned })}
            >
              {message.pinned ? m.messages_unpin() : m.messages_pin()}
            </Button>
            <DialogTrigger>
              <Button variant="ghost">{m.messages_delete()}</Button>
              <ModalOverlay className="overlay" isDismissable>
                <Modal className="modal">
                  <Dialog className="dialog" role="alertdialog">
                    {({ close }) => (
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
                    )}
                  </Dialog>
                </Modal>
              </ModalOverlay>
            </DialogTrigger>
          </div>
        )}
      </div>
    </article>
  );
}
