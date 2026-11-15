import { useQueries } from "@tanstack/react-query";
import { useState } from "react";
import { keys } from "../../app/query";
import { formatDateTime, formatDuration } from "../../i18n/format";
import { api, type CatchUp, type Member } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { Icon } from "../../ui/Icon";
import { MessageCard } from "./MessageCard";
import { names } from "./people";
import { useMarkRead } from "./useMessages";

/** Au-delà, les rattrapages sont repliés derrière un résumé. */
const FOLD_AFTER = 2;

const isEmpty = (d: CatchUp) => d.fronts.length === 0 && d.unread.length === 0;
/** Une notification par message non lu, ou une seule s'il n'y a que des passages. */
const countOf = (d: CatchUp) => Math.max(d.unread.length, isEmpty(d) ? 0 : 1);

/**
 * Rattrapages de tous les membres présents. Plus de deux notifications au
 * total : repliées en un résumé qu'on peut déplier, avec « Tout marquer comme lu ».
 */
export function CatchUpStack({ members, byId }: { members: Member[]; byId: Map<string, Member> }) {
  const queries = useQueries({
    queries: members.map((member) => ({
      queryKey: keys.catchUp(member.id),
      queryFn: () => api.messages.catchUp(member.id),
    })),
  });
  const markRead = useMarkRead();
  const [open, setOpen] = useState(false);

  const items = members.flatMap((member, i) => {
    const data = queries[i]?.data;
    return data && !isEmpty(data) ? [{ member, data }] : [];
  });
  if (items.length === 0) return null;
  const total = items.reduce((n, x) => n + countOf(x.data), 0);
  const folded = total > FOLD_AFTER;

  const markAll = async () => {
    for (const { member, data } of items) {
      await markRead.mutateAsync({ memberId: member.id, ids: data.unread.map((x) => x.id) });
    }
  };

  const panels = items.map(({ member, data }) => (
    <CatchUpPanel key={member.id} member={member} data={data} byId={byId} />
  ));
  if (!folded) return <>{panels}</>;

  return (
    <div className="panel catchup">
      <div className="catchup__summary">
        <div>
          <h2 className="panel__title">{m.catchup_folded_title({ count: total })}</h2>
          <p className="muted">
            {m.catchup_folded_for({
              names: names(
                items.map((x) => x.member.id),
                byId,
                4,
              ),
            })}
          </p>
        </div>
        <div className="catchup__summary-actions">
          <Button variant="ghost" onPress={() => setOpen((o) => !o)} aria-expanded={open}>
            <Icon name={open ? "chevron-up" : "chevron-down"} />
            {open ? m.catchup_fold() : m.catchup_unfold()}
          </Button>
          <Button variant="accent" isDisabled={markRead.isPending} onPress={() => void markAll()}>
            {m.messages_mark_all_read()}
          </Button>
        </div>
      </div>
      {open && <div className="catchup__stack">{panels}</div>}
    </div>
  );
}

/**
 * « Depuis ton dernier passage » : ce qui s'est passé pendant qu'un membre
 * n'était pas au front.
 */
function CatchUpPanel({
  member,
  data,
  byId,
}: {
  member: Member;
  data: CatchUp;
  byId: Map<string, Member>;
}) {
  const markRead = useMarkRead();

  // Temps cumulé au front par personne depuis le dernier passage.
  const now = Date.now();
  const totals = new Map<string, number>();
  for (const s of data.fronts) {
    const k = s.memberId ?? "";
    const start = Math.max(s.startedAt ?? now, data.lastSeen ?? 0);
    totals.set(k, (totals.get(k) ?? 0) + Math.max(0, (s.endedAt ?? now) - start));
  }

  return (
    <div className="panel catchup">
      <div>
        <h2 className="panel__title">{m.catchup_title({ name: member.name })}</h2>
        <p className="muted">
          {data.lastSeen
            ? m.catchup_last_seen({ when: formatDateTime(data.lastSeen) })
            : m.catchup_first()}
        </p>
      </div>
      {totals.size > 0 && (
        <div className="catchup__fronts">
          <span className="muted">{m.catchup_since()}</span>
          {[...totals].map(([id, ms]) => (
            <span key={id || "blurry"} className="tag">
              {(id && byId.get(id)?.name) || m.front_blurry()} · {formatDuration(ms)}
            </span>
          ))}
        </div>
      )}
      {data.unread.length > 0 && (
        <>
          <h3 className="catchup__subtitle">{m.catchup_unread()}</h3>
          <div className="messages">
            {data.unread.map((msg) => (
              <MessageCard key={msg.id} message={msg} byId={byId} actions={false} />
            ))}
          </div>
        </>
      )}
      <div>
        <Button
          variant="accent"
          isDisabled={markRead.isPending}
          onPress={() =>
            markRead.mutate({ memberId: member.id, ids: data.unread.map((x) => x.id) })
          }
        >
          {m.catchup_done()}
        </Button>
      </div>
    </div>
  );
}
