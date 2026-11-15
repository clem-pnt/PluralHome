import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { type CSSProperties, useState } from "react";
import { Dialog, Heading, Modal, ModalOverlay } from "react-aria-components";
import { keys } from "../../app/query";
import { formatDay, formatDuration, formatTime, startOfDay } from "../../i18n/format";
import { api, type FrontSession } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Avatar } from "../../ui/Avatar";
import { Button } from "../../ui/Button";
import { useMembers } from "../members/useMembers";
import { statusLabel } from "./useFront";

const DAY_MS = 24 * 60 * 60 * 1000;
const STEP_DAYS = 7;

export function FrontHistoryDialog() {
  return (
    <ModalOverlay className="overlay" isDismissable>
      <Modal className="modal modal--wide">
        <Dialog className="dialog">
          <History />
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}

/** Tous les passages au front, jour par jour, du plus récent au plus ancien. */
function History() {
  const [days, setDays] = useState(STEP_DAYS);
  const [today] = useState(() => startOfDay(Date.now()));
  const to = today + DAY_MS;
  const from = today - (days - 1) * DAY_MS;
  const history = useQuery({
    queryKey: keys.frontRange(from, to),
    queryFn: () => api.front.history(from, to),
    placeholderData: keepPreviousData,
  });
  const members = useMembers();
  const sessions = (history.data ?? [])
    .map((s) => ({ ...s, startedAt: s.startedAt ?? from }))
    .sort((a, b) => b.startedAt - a.startedAt);

  // Groupé par jour de début (borné au début de la période).
  const byDay = new Map<number, (FrontSession & { startedAt: number })[]>();
  for (const s of sessions) {
    const d = startOfDay(Math.max(s.startedAt, from));
    byDay.set(d, [...(byDay.get(d) ?? []), s]);
  }

  return (
    <>
      <div>
        <Heading slot="title">{m.front_history_title()}</Heading>
        <p className="muted">{m.front_history_since({ day: formatDay(from) })}</p>
      </div>
      <div className="history">
        {history.isSuccess && sessions.length === 0 && (
          <p className="muted">{m.front_history_empty()}</p>
        )}
        {[...byDay].map(([day, list]) => (
          <section key={day} className="history__day">
            <h3 className="history__date">{formatDay(day)}</h3>
            <ul className="history__list">
              {list.map((s) => {
                const member = s.memberId ? members.byId.get(s.memberId) : undefined;
                const end = s.endedAt ?? Date.now();
                return (
                  <li key={s.id} className="history__row">
                    {member ? (
                      <Avatar name={member.name} src={member.avatar} size={24} />
                    ) : (
                      <span
                        className="avatar avatar--blurry"
                        style={{ "--size": "24px" } as CSSProperties}
                        aria-hidden
                      />
                    )}
                    <span className="history__name">
                      {member?.name ?? m.front_blurry()}
                      {s.status !== "front" && (
                        <span className={`status status--${s.status}`}>
                          {statusLabel(s.status)}
                        </span>
                      )}
                    </span>
                    <span className="muted history__time">
                      {formatTime(s.startedAt)} – {s.endedAt ? formatTime(end) : m.front_ongoing()}
                      {" · "}
                      {formatDuration(end - s.startedAt)}
                    </span>
                  </li>
                );
              })}
            </ul>
          </section>
        ))}
      </div>
      <div className="dialog__actions">
        <Button
          variant="ghost"
          isDisabled={history.isFetching}
          onPress={() => setDays((d) => d + STEP_DAYS)}
        >
          {m.front_history_more()}
        </Button>
        <span style={{ flex: 1 }} />
        <Button slot="close">{m.close()}</Button>
      </div>
    </>
  );
}
