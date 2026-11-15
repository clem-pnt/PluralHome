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
    mutationFn: ({
      id,
      input,
      messageIds,
    }: {
      id: string | null;
      input: GroupInput;
      /** Messages d'une discussion qui devient ce groupe. */
      messageIds?: string[] | undefined;
    }) =>
      id
        ? api.groups.update(id, input)
        : messageIds?.length
          ? api.groups.createFrom(input, messageIds)
          : api.groups.create(input),
    onSuccess: (group, { messageIds }) => {
      upsert(group);
      if (messageIds?.length) void queryClient.invalidateQueries({ queryKey: keys.messages });
    },
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
