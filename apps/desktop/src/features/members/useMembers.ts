import { useQuery } from "@tanstack/react-query";
import { keys } from "../../app/query";
import { api, type Member } from "../../ipc";

export function useMembers() {
  const query = useQuery({ queryKey: keys.members, queryFn: api.members.list });
  const list = query.data ?? [];
  const byId = new Map<string, Member>(list.map((m) => [m.id, m]));
  return { ...query, list, byId };
}
