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

export const formatTime = (ms: number) => time.format(ms);
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
