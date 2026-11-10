import { DialogTrigger } from "react-aria-components";
import { errorMessage } from "../../i18n/errors";
import { formatDay, formatTime } from "../../i18n/format";
import { m } from "../../paraglide/messages.js";
import { Avatar } from "../../ui/Avatar";
import { Button } from "../../ui/Button";
import { useMembers } from "../members/useMembers";
import { FrontDialog } from "./FrontDialog";
import { Timeline } from "./Timeline";
import { useFrontChange, useFrontCurrent } from "./useFront";

export function FrontView() {
  const members = useMembers();
  const current = useFrontCurrent();
  const change = useFrontChange();
  const sessions = current.data ?? [];
  const fronting = new Set(sessions.map((s) => s.memberId));

  return (
    <section className="view">
      <header className="view__header">
        <div>
          <h1>{m.front_title()}</h1>
          <p className="view__lead">{formatDay(Date.now())}</p>
        </div>
        {members.list.length > 0 && (
          <DialogTrigger>
            <Button>{m.front_change()}</Button>
            <FrontDialog current={sessions} />
          </DialogTrigger>
        )}
      </header>

      {members.isSuccess && members.list.length === 0 ? (
        <div className="empty">
          <p>{m.front_no_members()}</p>
        </div>
      ) : (
        <>
          <div className="panel">
            <h2 className="panel__title">{m.front_now()}</h2>
            {sessions.length === 0 && current.isSuccess && (
              <p className="muted">{m.front_nobody()}</p>
            )}
            <ul className="fronters">
              {sessions.map((s) => {
                const member = s.memberId ? members.byId.get(s.memberId) : undefined;
                return (
                  <li key={s.id} className="fronter">
                    {member ? (
                      <Avatar name={member.name} color={member.color} size={48} />
                    ) : (
                      <span className="avatar avatar--blurry" aria-hidden />
                    )}
                    <span className="fronter__text">
                      <span className="fronter__name">{member?.name ?? m.front_blurry()}</span>
                      <span className="muted">
                        {m.front_since({ time: formatTime(s.startedAt ?? Date.now()) })}
                      </span>
                    </span>
                  </li>
                );
              })}
            </ul>
          </div>

          <div className="panel">
            <h2 className="panel__title">{m.front_quick()}</h2>
            <p className="muted">{m.front_quick_hint()}</p>
            <div className="quick">
              {members.list.map((member) => (
                <Button
                  key={member.id}
                  variant="ghost"
                  isDisabled={change.isPending}
                  aria-pressed={fronting.has(member.id) && sessions.length === 1}
                  onPress={() => change.mutate({ members: [member.id], blurry: false })}
                >
                  <Avatar name={member.name} color={member.color} size={22} />
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
