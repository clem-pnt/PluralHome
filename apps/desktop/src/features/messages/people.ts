import { formatList } from "../../i18n/format";
import type { FrontSession, Member, Message } from "../../ipc";
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

/** Membres présents, quel que soit leur statut (co-conscience comprise). */
export function presentIds(sessions: FrontSession[] | undefined): string[] {
  return (sessions ?? []).flatMap((s) => (s.memberId ? [s.memberId] : []));
}

/**
 * Qui peut marquer ce message comme lu : les destinataires au front qui ne l'ont
 * pas encore lu (tout le monde au front pour un message à tout le système, sauf
 * les auteurs). Sans personne au front, les destinataires directs.
 */
export function readersOf(message: Message, fronters: string[]): string[] {
  const { authorIds, recipientIds, readBy } = message;
  const broadcast = recipientIds.length === 0;
  const list =
    fronters.length > 0
      ? broadcast
        ? fronters.filter((id) => !authorIds.includes(id))
        : fronters.filter((id) => recipientIds.includes(id))
      : recipientIds;
  return list.filter((id) => !readBy.includes(id));
}

/** Un message ne s'affiche que s'il concerne quelqu'un de présent, ou tout le système. */
export function concerns(message: Message, present: string[]): boolean {
  return (
    message.recipientIds.length === 0 ||
    message.authorIds.some((id) => present.includes(id)) ||
    message.recipientIds.some((id) => present.includes(id))
  );
}

export const EVERYONE = "all";

/** Conversation d'un message : tout le système, ou l'ensemble auteurs + destinataires. */
export function conversationOf(message: Message): string {
  if (message.recipientIds.length === 0) return EVERYONE;
  return participantsKey([...message.authorIds, ...message.recipientIds]);
}

export function participantsKey(ids: string[]): string {
  return [...new Set(ids)].sort().join(",");
}
