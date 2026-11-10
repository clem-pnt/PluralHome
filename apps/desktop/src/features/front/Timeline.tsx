import { useQuery } from "@tanstack/react-query";
import { type CSSProperties, useEffect, useState } from "react";
import { keys } from "../../app/query";
import { formatTime, startOfDay } from "../../i18n/format";
import { api, type FrontSession } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { useMembers } from "../members/useMembers";

const DAY_MS = 24 * 60 * 60 * 1000;
const HOURS = [0, 6, 12, 18, 24];

/** Rafraîchit `now` chaque minute pour faire avancer les intervalles ouverts. */
function useNow(stepMs = 60_000) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), stepMs);
    return () => window.clearInterval(id);
  }, [stepMs]);
  return now;
}

export function Timeline() {
  const now = useNow();
  const from = startOfDay(now);
  const to = from + DAY_MS;
  const history = useQuery({
    queryKey: keys.frontHistory(from),
    queryFn: () => api.front.history(from, to),
  });
  const members = useMembers();
  const sessions = history.data ?? [];

  if (history.isSuccess && sessions.length === 0) {
    return <p className="muted">{m.front_today_empty()}</p>;
  }

  // Une ligne par personne (ou le flou), dans l'ordre d'apparition.
  const rows = new Map<string, FrontSession[]>();
  for (const s of sessions) {
    const k = s.memberId ?? "";
    rows.set(k, [...(rows.get(k) ?? []), s]);
  }
  const pct = (t: number) =>
    `${(((Math.min(Math.max(t, from), to) - from) / DAY_MS) * 100).toFixed(3)}%`;

  return (
    <div className="timeline">
      <div className="timeline__scale" aria-hidden>
        {HOURS.map((h) => (
          <span key={h} style={{ insetInlineStart: `${(h / 24) * 100}%` }}>
            {String(h).padStart(2, "0")} h
          </span>
        ))}
      </div>
      {[...rows].map(([memberId, list]) => {
        const member = memberId ? members.byId.get(memberId) : undefined;
        const name = member?.name ?? m.front_blurry();
        return (
          <div key={memberId || "blurry"} className="timeline__row">
            <span className="timeline__label">{name}</span>
            <div className="timeline__track">
              {list.map((s) => {
                const start = s.startedAt ?? from;
                const end = s.endedAt ?? now;
                return (
                  <span
                    key={s.id}
                    className={member ? "timeline__bar" : "timeline__bar timeline__bar--blurry"}
                    title={`${name} · ${formatTime(start)} – ${s.endedAt ? formatTime(end) : "…"}`}
                    style={
                      {
                        "--member": member?.color ?? "var(--ink-muted)",
                        insetInlineStart: pct(start),
                        inlineSize: `max(3px, calc(${pct(end)} - ${pct(start)}))`,
                      } as CSSProperties
                    }
                  />
                );
              })}
              <span className="timeline__now" style={{ insetInlineStart: pct(now) }} />
            </div>
          </div>
        );
      })}
    </div>
  );
}
