import { m } from "../paraglide/messages.js";
import { getLocale } from "../paraglide/runtime.js";

const time = new Intl.DateTimeFormat(getLocale(), { hour: "2-digit", minute: "2-digit" });
const day = new Intl.DateTimeFormat(getLocale(), {
  weekday: "long",
  day: "numeric",
  month: "long",
});

const dateTime = new Intl.DateTimeFormat(getLocale(), {
  weekday: "short",
  day: "numeric",
  month: "short",
  hour: "2-digit",
  minute: "2-digit",
});

const shortDay = new Intl.DateTimeFormat(getLocale(), {
  weekday: "short",
  day: "numeric",
  month: "short",
});

export const formatTime = (ms: number) => time.format(ms);
export const formatShortDay = (ms: number) => shortDay.format(ms);

/** « depuis aujourd'hui, 9:12 », « depuis hier, … », ou le jour en clair. */
export function formatSince(ms: number): string {
  const today = startOfDay(Date.now());
  const t = time.format(ms);
  if (ms >= today) return m.front_since_today({ time: t });
  if (ms >= startOfDay(today - 1)) return m.front_since_yesterday({ time: t });
  return m.front_since_day({ day: shortDay.format(ms), time: t });
}
export const formatDateTime = (ms: number) => dateTime.format(ms);

/** Heure seule si c'est aujourd'hui, sinon date et heure. */
export const formatWhen = (ms: number) =>
  startOfDay(ms) === startOfDay(Date.now()) ? time.format(ms) : dateTime.format(ms);
export const formatDay = (ms: number) => day.format(ms);

/** Minuit local du jour contenant `ms`. */
export function startOfDay(ms: number): number {
  const d = new Date(ms);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

export function formatDuration(ms: number): string {
  const total = Math.max(1, Math.round(ms / 60_000));
  const h = Math.floor(total / 60);
  const min = total % 60;
  return h > 0 ? m.duration_hm({ h, m: String(min).padStart(2, "0") }) : m.duration_m({ m: min });
}

const list = new Intl.ListFormat(getLocale(), { type: "conjunction" });
/** « Alice, Lou et Noa ». */
export const formatList = (items: string[]) => list.format(items);
