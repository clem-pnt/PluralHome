import { useState } from "react";
import { Button as AriaButton, Dialog, Heading, Modal, ModalOverlay } from "react-aria-components";
import { errorMessage } from "../../i18n/errors";
import { formatWhen } from "../../i18n/format";
import type { Member, Poll } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Avatar } from "../../ui/Avatar";
import { Button } from "../../ui/Button";
import { Icon } from "../../ui/Icon";
import { IconButton } from "../../ui/IconButton";
import { names } from "../messages/people";
import { useDeletePoll, useSetPollClosed, useVote } from "./usePolls";

export function PollCard({
  poll,
  byId,
  voters,
}: {
  poll: Poll;
  byId: Map<string, Member>;
  /** Qui vote en ce moment : le choix s'applique à chacun. */
  voters: string[];
}) {
  const vote = useVote();
  const setClosed = useSetPollClosed();
  const remove = useDeletePoll();
  const [confirm, setConfirm] = useState(false);

  const closed = poll.closedAt != null;
  const everyone = new Set(poll.options.flatMap((o) => o.voters));
  const top = Math.max(0, ...poll.options.map((o) => o.voters.length));
  // Une option est « choisie » quand toutes les personnes qui votent l'ont prise.
  const chosen = (voterIds: string[]) =>
    voters.length > 0 && voters.every((v) => voterIds.includes(v));
  const canVote = !closed && voters.length > 0 && !vote.isPending;

  const toggle = (id: string, on: boolean) => {
    const current = poll.options.filter((o) => chosen(o.voters)).map((o) => o.id);
    const options = poll.multi
      ? on
        ? current.filter((x) => x !== id)
        : [...current, id]
      : on
        ? []
        : [id];
    vote.mutate({ id: poll.id, voters, options });
  };

  const author = poll.authorId ? byId.get(poll.authorId) : undefined;

  return (
    <article className="poll panel" data-closed={closed || undefined}>
      <header className="poll__head">
        <div className="poll__title">
          <h3>{poll.question}</h3>
          <p className="poll__meta muted">
            {author && <span>{m.vote_by({ name: author.name })}</span>}
            <time dateTime={new Date(poll.createdAt ?? 0).toISOString()}>
              {formatWhen(poll.createdAt ?? 0)}
            </time>
            {poll.multi && <span className="tag">{m.vote_tag_multi()}</span>}
            {closed && <span className="tag">{m.vote_tag_closed()}</span>}
          </p>
        </div>
        <div className="poll__actions">
          <IconButton
            icon={closed ? "unlock" : "lock"}
            label={closed ? m.vote_reopen() : m.vote_close()}
            isDisabled={setClosed.isPending}
            onPress={() => setClosed.mutate({ id: poll.id, closed: !closed })}
          />
          <IconButton
            icon="trash"
            tone="danger"
            label={m.vote_delete()}
            onPress={() => setConfirm(true)}
          />
        </div>
      </header>
      <ul className="poll__options">
        {poll.options.map((o) => {
          const on = chosen(o.voters);
          const share = everyone.size ? o.voters.length / everyone.size : 0;
          const lead = closed && top > 0 && o.voters.length === top;
          return (
            <li key={o.id}>
              <AriaButton
                className="poll-option"
                aria-pressed={on}
                data-lead={lead || undefined}
                isDisabled={!canVote}
                onPress={() => toggle(o.id, on)}
                style={{ "--share": `${Math.round(share * 100)}%` } as React.CSSProperties}
              >
                <span className="poll-option__mark" aria-hidden>
                  {on && <Icon name="check" size={16} />}
                </span>
                <span className="poll-option__label">{o.label}</span>
                <span className="poll-option__faces" aria-hidden>
                  {o.voters.slice(0, 4).map((id) => {
                    const x = byId.get(id);
                    return x ? <Avatar key={id} name={x.name} src={x.avatar} size={22} /> : null;
                  })}
                </span>
                <span className="poll-option__count">
                  {m.vote_count({ count: o.voters.length })}
                </span>
              </AriaButton>
              {o.voters.length > 0 && (
                <p className="poll-option__who muted">{names(o.voters, byId, 5)}</p>
              )}
            </li>
          );
        })}
      </ul>
      <p className="muted poll__foot">
        {closed
          ? m.vote_closed_on({ when: formatWhen(poll.closedAt ?? 0) })
          : voters.length === 0
            ? m.vote_pick_voters_hint()
            : m.vote_voting_as({ names: names(voters, byId) })}
      </p>
      {(vote.isError || setClosed.isError) && (
        <p className="alert" role="alert">
          {errorMessage(vote.error ?? setClosed.error)}
        </p>
      )}
      <ModalOverlay className="overlay" isDismissable isOpen={confirm} onOpenChange={setConfirm}>
        <Modal className="modal">
          <Dialog className="dialog" role="alertdialog">
            {({ close }) => (
              <>
                <Heading slot="title">{m.vote_delete_title()}</Heading>
                <p className="muted">{m.vote_delete_body()}</p>
                <div className="dialog__actions">
                  <Button variant="ghost" slot="close">
                    {m.cancel()}
                  </Button>
                  <Button
                    variant="danger"
                    autoFocus
                    onPress={() => remove.mutate(poll.id, { onSuccess: close })}
                  >
                    {m.vote_delete()}
                  </Button>
                </div>
              </>
            )}
          </Dialog>
        </Modal>
      </ModalOverlay>
    </article>
  );
}
