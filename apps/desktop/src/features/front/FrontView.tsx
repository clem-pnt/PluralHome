import { DialogTrigger } from "react-aria-components";
import { errorMessage } from "../../i18n/errors";
import { formatDay, formatSince } from "../../i18n/format";
import type { FrontSession } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Avatar } from "../../ui/Avatar";
import { Button } from "../../ui/Button";
import { Icon } from "../../ui/Icon";
import { IconButton } from "../../ui/IconButton";
import { useMembers } from "../members/useMembers";
import { CatchUpPanel } from "../messages/CatchUpPanel";
import { FavoritesDialog } from "./FavoritesDialog";
import { FrontDialog } from "./FrontDialog";
import { FrontHistoryDialog } from "./FrontHistoryDialog";
import { Timeline } from "./Timeline";
import { STATUSES, statusLabel, useFrontChange, useFrontCurrent, useFrontQuick } from "./useFront";

export function FrontView() {
  const members = useMembers();
  const current = useFrontCurrent();
  const change = useFrontChange();
  const quick = useFrontQuick();
  const sessions = current.data ?? [];
  const fronting = new Set(sessions.map((s) => s.memberId));
  const [quickIds, chosen] = quick.data ?? [[], false];
  const quickMembers = quickIds.flatMap((id) => members.byId.get(id) ?? []);
  // Co-conscients à part : le flou compte comme « au front ».
  const groups = STATUSES.map(
    (st) => [st, sessions.filter((s) => s.status === st)] as const,
  ).filter(([, list]) => list.length > 0);

  return (
    <section className="view">
      <header className="view__header">
        <div>
          <h1>{m.front_title()}</h1>
          <p className="view__lead">{formatDay(Date.now())}</p>
        </div>
        <div className="view__actions">
          <DialogTrigger>
            <Button variant="ghost">
              <Icon name="history" size={18} />
              {m.front_history_open()}
            </Button>
            <FrontHistoryDialog />
          </DialogTrigger>
          {members.list.length > 0 && (
            <DialogTrigger>
              <Button>{m.front_change()}</Button>
              <FrontDialog current={sessions} />
            </DialogTrigger>
          )}
        </div>
      </header>

      {members.isSuccess && members.list.length === 0 ? (
        <div className="empty">
          <p>{m.front_no_members()}</p>
        </div>
      ) : (
        <>
          {sessions.map((s) => {
            const member = s.memberId ? members.byId.get(s.memberId) : undefined;
            return member ? <CatchUpPanel key={s.id} member={member} byId={members.byId} /> : null;
          })}
          <div className="panel">
            <h2 className="panel__title">{m.front_now()}</h2>
            {sessions.length === 0 && current.isSuccess && (
              <p className="muted">{m.front_nobody()}</p>
            )}
            {groups.map(([st, list]) => (
              <div key={st} className="fronters-group">
                {(groups.length > 1 || st !== "front") && (
                  <h3 className={`status status--${st}`}>{statusLabel(st)}</h3>
                )}
                <ul className="fronters">
                  {list.map((s) => (
                    <Fronter
                      key={s.id}
                      session={s}
                      member={s.memberId ? members.byId.get(s.memberId) : undefined}
                    />
                  ))}
                </ul>
              </div>
            ))}
          </div>

          <div className="panel">
            <div className="panel__head">
              <h2 className="panel__title">{m.front_quick()}</h2>
              <DialogTrigger>
                <IconButton icon="star" label={m.front_favorites_edit()} />
                <FavoritesDialog initial={chosen ? quickIds : []} />
              </DialogTrigger>
            </div>
            {quick.isSuccess && (
              <p className="muted">
                {chosen
                  ? m.front_quick_hint()
                  : quickMembers.length > 0
                    ? m.front_quick_frequent()
                    : m.front_quick_none()}
              </p>
            )}
            <div className="quick">
              {quickMembers.map((member) => (
                <Button
                  key={member.id}
                  variant="ghost"
                  isDisabled={change.isPending}
                  aria-pressed={fronting.has(member.id) && sessions.length === 1}
                  onPress={() => change.mutate({ members: [member.id], blurry: false })}
                >
                  <Avatar name={member.name} src={member.avatar} size={22} />
                  {member.name}
                </Button>
              ))}
              <Button
                variant="ghost"
                isDisabled={change.isPending}
                onPress={() => change.mutate({ members: [], blurry: true })}
              >
                <span
                  className="avatar avatar--blurry"
                  style={{ "--size": "22px" } as React.CSSProperties}
                  aria-hidden
                />
                {m.front_blurry()}
              </Button>
            </div>
            {change.isError && (
              <p className="alert" role="alert">
                {errorMessage(change.error)}
              </p>
            )}
          </div>

          <div className="panel">
            <h2 className="panel__title">{m.front_today()}</h2>
            <Timeline />
          </div>
        </>
      )}
    </section>
  );
}

function Fronter({
  session,
  member,
}: {
  session: FrontSession;
  member: { name: string; avatar?: string | null } | undefined;
}) {
  return (
    <li className="fronter">
      {member ? (
        <Avatar name={member.name} src={member.avatar} size={48} />
      ) : (
        <span className="avatar avatar--blurry" aria-hidden />
      )}
      <span className="fronter__text">
        <span className="fronter__name">{member?.name ?? m.front_blurry()}</span>
        <span className="muted">{formatSince(session.startedAt ?? Date.now())}</span>
      </span>
    </li>
  );
}
