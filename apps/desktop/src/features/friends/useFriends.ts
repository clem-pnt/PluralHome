import { useQuery } from "@tanstack/react-query";
import { keys, queryClient } from "../../app/query";
import { AppError, api, type Friend } from "../../ipc";
import { m } from "../../paraglide/messages.js";

export function useFriends() {
  const query = useQuery({ queryKey: keys.friendsList, queryFn: api.friends.list });
  const list = query.data ?? [];
  const byId = new Map<string, Friend>(list.map((f) => [f.id, f]));
  return { ...query, list, byId };
}

/** Nom affiché d'un ami : le surnom qu'on lui donne, sinon le nom de son système. */
export function friendName(f: Pick<Friend, "nickname" | "name">): string {
  return f.nickname?.trim() || f.name?.trim() || m.friends_unnamed();
}

/** Après toute action sur les amis : la liste, les demandes et les messages partagés. */
export function refreshFriends() {
  void queryClient.invalidateQueries({ queryKey: keys.friends });
  void queryClient.invalidateQueries({ queryKey: keys.messages });
  void queryClient.invalidateQueries({ queryKey: keys.groups });
}

/** Un autre appareil du compte porte déjà une identité d'amis différente. */
export function isIdentityConflict(e: unknown): boolean {
  return e instanceof AppError && e.code === "friend_identity_conflict";
}
