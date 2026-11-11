import { useInfiniteQuery, useMutation } from "@tanstack/react-query";
import { keys, queryClient } from "../../app/query";
import { api, type NewEntry } from "../../ipc";

const PAGE = 30;

export function useJournal(author: string | null) {
  return useInfiniteQuery({
    queryKey: keys.journal(author),
    queryFn: ({ pageParam }) => api.journal.list(pageParam, author, PAGE),
    initialPageParam: null as string | null,
    getNextPageParam: (last) => (last.length === PAGE ? (last.at(-1)?.id ?? null) : null),
  });
}

const refresh = () => void queryClient.invalidateQueries({ queryKey: ["journal"] });

export const useSaveEntry = () =>
  useMutation({
    mutationFn: ({ id, input }: { id: string | null; input: NewEntry }) =>
      id ? api.journal.update(id, input) : api.journal.create(input),
    onSuccess: refresh,
  });

export const useDeleteEntry = () =>
  useMutation({ mutationFn: api.journal.remove, onSuccess: refresh });
