import { useQuery } from "@tanstack/react-query";
import { keys, queryClient } from "../../app/query";
import { api, type Folder } from "../../ipc";

export function useFolders() {
  const query = useQuery({ queryKey: keys.folders, queryFn: api.folders.list });
  return { ...query, list: query.data ?? [] };
}

export const setFolders = (next: Folder[]) => queryClient.setQueryData(keys.folders, next);

/** Dossiers où est rangé un membre. */
export const foldersOf = (folders: Folder[], memberId: string) =>
  folders.filter((f) => f.members.includes(memberId));
