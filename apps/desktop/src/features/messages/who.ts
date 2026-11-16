import { formatList } from "../../i18n/format";
import type { Friend, Member, Message, RemoteRef, SystemProfile } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { friendName, useFriends } from "../friends/useFriends";
import { useMembers } from "../members/useMembers";
import { names } from "./people";
import { useSystem } from "./useGroups";

/** De quoi nommer les deux côtés d'un message : nos membres, nos amis, notre système. */
export interface Who {
  byId: Map<string, Member>;
  friends: Map<string, Friend>;
  system: SystemProfile | undefined;
}

export function useWho(): Who {
  const members = useMembers();
  const friends = useFriends();
  const system = useSystem();
  return { byId: members.byId, friends: friends.byId, system: system.data };
}

export type Face = { name: string; avatar: string | null };

/** Membres d'un ami, d'après les noms gardés avec le message et sa fiche. */
export function remoteFaces(refs: RemoteRef[], friend: Friend | undefined): Face[] {
  return refs.map((r) => {
    const card = friend?.members.find((x) => x.id === r.ref);
    return { name: card?.name ?? r.name, avatar: card?.avatar ?? null };
  });
}

export function remoteNames(refs: RemoteRef[], friend: Friend | undefined, max = 3): string {
  const list = remoteFaces(refs, friend).map((x) => x.name);
  if (list.length <= max) return formatList(list);
  return m.messages_people_more({
    names: list.slice(0, max - 1).join(", "),
    count: list.length - (max - 1),
  });
}

export const friendLabel = (friend: Friend | undefined) =>
  friend ? friendName(friend) : m.messages_friend_gone();

export const systemLabel = (system: SystemProfile | undefined) =>
  system?.name?.trim() || m.messages_everyone();

/** Signature d'un message : ses auteurs, de chez nous ou de chez l'ami. */
export function authorLabel(message: Message, who: Who, max = 3): string {
  if (message.friendId && !message.outgoing) {
    // Relayé par l'ami depuis un autre système du groupe : on nomme ce dernier.
    const friend = message.system ? undefined : who.friends.get(message.friendId);
    const list = remoteNames(message.remoteAuthors, friend, max);
    const sys = message.system?.name ?? friendLabel(friend);
    return list ? m.messages_friend_people({ names: list, system: sys }) : sys;
  }
  if (message.bySystem) return systemLabel(who.system);
  return names(message.authorIds, who.byId, max) || m.messages_someone();
}

/** Visage de l'auteur principal. */
export function authorFace(message: Message, who: Who): Face | null {
  if (message.friendId && !message.outgoing) {
    const friend = message.system ? undefined : who.friends.get(message.friendId);
    const first = remoteFaces(message.remoteAuthors.slice(0, 1), friend)[0];
    return (
      first ?? {
        name: message.system?.name ?? friendLabel(friend),
        avatar: friend?.avatar ?? null,
      }
    );
  }
  if (message.bySystem)
    return { name: systemLabel(who.system), avatar: who.system?.avatar ?? null };
  const x = who.byId.get(message.authorIds[0] ?? "");
  return x ? { name: x.name, avatar: x.avatar ?? null } : null;
}

/** Pour regrouper les messages successifs d'une même signature. */
export const authorKey = (message: Message) =>
  [
    message.outgoing ? "o" : "i",
    message.system?.key ?? "",
    message.bySystem ? "s" : "",
    ...message.authorIds,
    ...message.remoteAuthors.map((r) => r.ref),
  ].join();

/** Destinataires d'un message, des deux côtés. */
export function recipientLabel(message: Message, who: Who): string {
  if (message.friendId && message.outgoing) {
    const friend = who.friends.get(message.friendId);
    const list = remoteNames(message.remoteRecipients, friend);
    const sys = friendLabel(friend);
    return list ? m.messages_friend_people({ names: list, system: sys }) : sys;
  }
  if (message.recipientIds.length === 0) return m.messages_everyone_route();
  return names(message.recipientIds, who.byId);
}
