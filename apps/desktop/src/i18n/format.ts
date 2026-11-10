import { getLocale } from "../paraglide/runtime.js";

const time = new Intl.DateTimeFormat(getLocale(), { hour: "2-digit", minute: "2-digit" });
const day = new Intl.DateTimeFormat(getLocale(), {
  weekday: "long",
  day: "numeric",
  month: "long",
});

export const formatTime = (ms: number) => time.format(ms);
export const formatDay = (ms: number) => day.format(ms);

/** Minuit local du jour contenant `ms`. */
export function startOfDay(ms: number): number {
  const d = new Date(ms);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}
