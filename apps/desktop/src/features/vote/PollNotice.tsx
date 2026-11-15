import { useNavigate } from "../../app/nav";
import { formatDeadline } from "../../i18n/format";
import type { Member, Poll } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { names } from "../messages/people";
import { useMarkResultSeen } from "./usePolls";

/** Membres présents qui n'ont pas encore répondu à ce vote. */
export const notVoted = (p: Poll, present: string[]) => {
  const done = new Set(p.options.flatMap((o) => o.voters));
  return present.filter((id) => !done.has(id));
};

/**
 * Ce qui mérite une notification : les votes en cours où quelqu'un de présent
 * n'a pas répondu, et les votes terminés dont le résultat n'a pas été vu.
 */
export function pollAlerts(polls: Poll[], present: string[]) {
  return {
    pending: polls.filter((p) => p.closedAt == null && notVoted(p, present).length > 0),
    ended: polls.filter((p) => p.closedAt != null && !p.resultSeen),
  };
}

/** Option(s) en tête d'un vote. */
function winners(p: Poll): string {
  const top = Math.max(0, ...p.options.map((o) => o.voters.length));
  if (top === 0) return m.vote_notice_no_votes();
  return m.vote_notice_result({
    options: p.options
      .filter((o) => o.voters.length === top)
      .map((o) => o.label)
      .join(" / "),
    count: top,
  });
}

/** Notification de la page Front : votes en cours et résultats à voir. */
export function PollNotice({
  polls,
  present,
  byId,
}: {
  polls: Poll[];
  present: string[];
  byId: Map<string, Member>;
}) {
  const navigate = useNavigate();
  const seen = useMarkResultSeen();
  const { pending, ended } = pollAlerts(polls, present);
  if (pending.length === 0 && ended.length === 0) return null;

  return (
    <div className="panel catchup poll-notice">
      {pending.length > 0 && (
        <div className="poll-notice__group">
          <h2 className="panel__title">{m.vote_notice_pending({ count: pending.length })}</h2>
          <ul className="poll-notice__list">
            {pending.map((p) => (
              <li key={p.id}>
                <strong>{p.question}</strong>
                <span className="muted">
                  {m.vote_notice_missing({ names: names(notVoted(p, present), byId, 4) })}
                  {p.endsAt != null && ` · ${m.vote_ends_on({ when: formatDeadline(p.endsAt) })}`}
                </span>
              </li>
            ))}
          </ul>
        </div>
      )}
      {ended.length > 0 && (
        <div className="poll-notice__group">
          <h2 className="panel__title">{m.vote_notice_ended({ count: ended.length })}</h2>
          <ul className="poll-notice__list">
            {ended.map((p) => (
              <li key={p.id}>
                <strong>{p.question}</strong>
                <span className="muted">{winners(p)}</span>
              </li>
            ))}
          </ul>
        </div>
      )}
      <div className="catchup__summary-actions">
        <Button variant="accent" onPress={() => navigate("vote")}>
          {m.vote_notice_open()}
        </Button>
        {ended.length > 0 && (
          <Button
            variant="ghost"
            isDisabled={seen.isPending}
            onPress={() => {
              for (const p of ended) seen.mutate(p.id);
            }}
          >
            {m.vote_notice_dismiss()}
          </Button>
        )}
      </div>
    </div>
  );
}
