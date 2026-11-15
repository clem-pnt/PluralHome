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
 * pas encore lu (tout le monde au front pour un message à tout le système). Sans
 * personne au front, les destinataires directs. Jamais les auteurs.
 */
export function readersOf(message: Message, fronters: string[]): string[] {
  // Ce qu'on envoie à un ami se lit chez lui, pas chez nous.
  if (message.friendId && message.outgoing) return [];
  const { authorIds, recipientIds, readBy } = message;
  const broadcast = recipientIds.length === 0;
  const list =
    fronters.length > 0
      ? broadcast
        ? fronters
        : fronters.filter((id) => recipientIds.includes(id))
      : recipientIds;
  // Ceux qui ont écrit le message n'ont rien à lire, même s'ils en sont aussi
  // destinataires (plusieurs alters qui s'écrivent entre eux).
  return list.filter((id) => !authorIds.includes(id) && !readBy.includes(id));
}

/** Un message ne s'affiche que s'il concerne quelqu'un de présent, ou tout le système. */
export function concerns(message: Message, present: string[]): boolean {
  if (message.friendId) {
    // Avec un ami, notre côté est l'auteur (envoyé) ou le destinataire (reçu).
    const ours = ourSide(message);
    return ours.length === 0 || ours.some((id) => present.includes(id));
  }
  return (
    message.recipientIds.length === 0 ||
    message.authorIds.some((id) => present.includes(id)) ||
    message.recipientIds.some((id) => present.includes(id))
  );
}

export const EVERYONE = "all";

/** Clé de conversation d'un groupe nommé. */
export const groupKey = (id: string) => `g:${id}`;

/**
 * Conversation d'un message : son groupe nommé, tout le système, ou l'ensemble
 * auteurs + destinataires.
 */
export function conversationOf(message: Message): string {
  if (message.groupId) return groupKey(message.groupId);
  if (message.friendId) return friendKey(message.friendId, ourSide(message), theirSide(message));
  if (message.recipientIds.length === 0) return EVERYONE;
  return participantsKey([...message.authorIds, ...message.recipientIds]);
}

export function participantsKey(ids: string[]): string {
  return [...new Set(ids)].sort().join(",");
}

/** Nos membres dans un échange avec un ami : vide pour tout le système. */
export function ourSide(message: Message): string[] {
  return message.outgoing ? message.authorIds : message.recipientIds;
}

/** Les membres de l'ami dans l'échange : vide pour tout son système. */
export function theirSide(message: Message): string[] {
  return (message.outgoing ? message.remoteRecipients : message.remoteAuthors).map((r) => r.ref);
}

/**
 * Discussion avec un ami : ses participants des deux côtés. Les réponses
 * retombent dans la même discussion, puisque les côtés s'inversent.
 */
export function friendKey(friendId: string, ours: string[], theirs: string[]): string {
  return `f:${friendId}:${participantsKey(ours)}|${participantsKey(theirs)}`;
}
