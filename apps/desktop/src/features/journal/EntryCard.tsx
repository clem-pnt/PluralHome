import { Dialog, DialogTrigger, Heading, Modal, ModalOverlay } from "react-aria-components";
import { formatTime } from "../../i18n/format";
import type { Entry, Member } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Avatar } from "../../ui/Avatar";
import { Button } from "../../ui/Button";
import { EntryForm } from "./EntryForm";
import { moodLabel } from "./mood";
import { useDeleteEntry } from "./useJournal";

export function EntryCard({ entry, byId }: { entry: Entry; byId: Map<string, Member> }) {
  const authors = entry.authorIds.flatMap((id) => {
    const a = byId.get(id);
    return a ? [a] : [];
  });
  const remove = useDeleteEntry();

  return (
    <article className="entry">
      <header className="entry__meta">
        <span className="entry__authors">
          {authors.length > 0 ? (
            authors.map((a) => (
              <span key={a.id} className="entry__author">
                <Avatar name={a.name} color={a.color} src={a.avatar} size={22} />
                {a.name}
              </span>
            ))
          ) : (
            <span className="muted">{m.messages_someone()}</span>
          )}
        </span>
        <time className="muted" dateTime={new Date(entry.createdAt ?? 0).toISOString()}>
          {formatTime(entry.createdAt ?? 0)}
        </time>
        {entry.updatedAt && <span className="muted">{m.journal_edited()}</span>}
        {entry.mood && (
          <span className="tag" data-mood={entry.mood}>
            {moodLabel(entry.mood)}
          </span>
        )}
      </header>
      {entry.title && <h3 className="entry__title">{entry.title}</h3>}
      <p className="message__body" data-selectable>
        {entry.body}
      </p>
      <div className="message__actions">
        <DialogTrigger>
          <Button variant="ghost">{m.member_edit()}</Button>
          <ModalOverlay className="overlay" isDismissable>
            <Modal className="modal modal--wide">
              <Dialog className="dialog">
                {({ close }) => (
                  <>
                    <Heading slot="title">{m.journal_edit_title()}</Heading>
                    <EntryForm entry={entry} onDone={close} />
                  </>
                )}
              </Dialog>
            </Modal>
          </ModalOverlay>
        </DialogTrigger>
        <DialogTrigger>
          <Button variant="ghost">{m.messages_delete()}</Button>
          <ModalOverlay className="overlay" isDismissable>
            <Modal className="modal">
              <Dialog className="dialog" role="alertdialog">
                {({ close }) => (
                  <>
                    <Heading slot="title">{m.journal_delete_title()}</Heading>
                    <p className="muted">{m.journal_delete_body()}</p>
                    <div className="dialog__actions">
                      <Button variant="ghost" slot="close">
                        {m.cancel()}
                      </Button>
                      <Button
                        variant="danger"
                        autoFocus
                        isDisabled={remove.isPending}
                        onPress={() => remove.mutate(entry.id, { onSuccess: close })}
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
    </article>
  );
}
