import { formatList } from "../../i18n/format";
import type { FrontSession, Member } from "../../ipc";
import { m } from "../../paraglide/messages.js";

/** Noms lisibles d'une liste de membres, abrégée au-delà de trois. */
export function names(ids: string[], byId: Map<string, Member>, max = 3): string {
  const known = ids.flatMap((id) => {
    const x = byId.get(id);
    return x ? [x.name] : [];
  });
  if (known.length <= max) return formatList(known);
  return m.messages_people_more({
    names: known.slice(0, max - 1).join(", "),
    count: known.length - (max - 1),
  });
}

/** Membres vraiment au front (front et co-front, pas la co-conscience). */
export function fronting(sessions: FrontSession[] | undefined): string[] {
  return (sessions ?? []).flatMap((s) =>
    s.memberId && s.status !== "coconscious" ? [s.memberId] : [],
  );
}
