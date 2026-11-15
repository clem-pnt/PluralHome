import { useMutation, useQuery } from "@tanstack/react-query";
import { keys, queryClient } from "../../app/query";
import { api, type ChatGroup, type GroupInput } from "../../ipc";

export const useGroups = () => useQuery({ queryKey: keys.groups, queryFn: api.groups.list });

const upsert = (group: ChatGroup) =>
  queryClient.setQueryData<ChatGroup[]>(keys.groups, (prev = []) => [
    ...prev.filter((g) => g.id !== group.id),
    group,
  ]);

export const useSaveGroup = () =>
  useMutation({
    mutationFn: ({ id, input }: { id: string | null; input: GroupInput }) =>
      id ? api.groups.update(id, input) : api.groups.create(input),
    onSuccess: upsert,
  });

/** Supprime le groupe et ses messages. */
export const useDeleteGroup = () =>
  useMutation({
    mutationFn: api.groups.remove,
    onSuccess: (_, id) => {
      queryClient.setQueryData<ChatGroup[]>(keys.groups, (prev = []) =>
        prev.filter((g) => g.id !== id),
      );
      void queryClient.invalidateQueries({ queryKey: keys.messages });
      void queryClient.invalidateQueries({ queryKey: ["catch-up"] });
    },
  });

/** Nom et photo du système (PluralKit ou paramètres). */
export const useSystem = () => useQuery({ queryKey: keys.system, queryFn: api.system.get });
