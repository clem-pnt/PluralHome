import { useQuery } from "@tanstack/react-query";
import { keys } from "../../app/query";
import { formatDateTime, formatDuration } from "../../i18n/format";
import { api, type Member } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { MessageCard } from "./MessageCard";
import { useMarkRead } from "./useMessages";

/**
 * « Depuis ton dernier passage » : ce qui s'est passé pendant qu'un membre
 * n'était pas au front. N'apparaît que s'il y a quelque chose à rattraper.
 */
export function CatchUpPanel({ member, byId }: { member: Member; byId: Map<string, Member> }) {
  const query = useQuery({
    queryKey: keys.catchUp(member.id),
    queryFn: () => api.messages.catchUp(member.id),
  });
  const markRead = useMarkRead();
  const data = query.data;
  if (!data || (data.fronts.length === 0 && data.unread.length === 0)) return null;

  // Temps cumulé au front par personne depuis le dernier passage.
  const now = Date.now();
  const totals = new Map<string, number>();
  for (const s of data.fronts) {
    const k = s.memberId ?? "";
    const start = Math.max(s.startedAt ?? now, data.lastSeen ?? 0);
    totals.set(k, (totals.get(k) ?? 0) + Math.max(0, (s.endedAt ?? now) - start));
  }

  return (
    <div className="panel catchup" style={{ "--member": member.color } as React.CSSProperties}>
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
