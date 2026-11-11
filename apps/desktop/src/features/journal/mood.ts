import { m } from "../../paraglide/messages.js";

export const MOODS = [
  { value: 1, label: m.mood_1 },
  { value: 2, label: m.mood_2 },
  { value: 3, label: m.mood_3 },
  { value: 4, label: m.mood_4 },
  { value: 5, label: m.mood_5 },
] as const;

export const moodLabel = (mood: number) => MOODS.find((x) => x.value === mood)?.label() ?? "";
