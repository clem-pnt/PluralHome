import { useInfiniteQuery, useMutation } from "@tanstack/react-query";
import { keys, queryClient } from "../../app/query";
import { api } from "../../ipc";

const PAGE = 50;

export function useMessageBoard() {
  return useInfiniteQuery({
    queryKey: keys.messages,
    queryFn: ({ pageParam }) => api.messages.list(pageParam, PAGE),
    initialPageParam: null as string | null,
    getNextPageParam: (last) => (last.length === PAGE ? (last.at(-1)?.id ?? null) : null),
  });
}

const refresh = () => {
  void queryClient.invalidateQueries({ queryKey: keys.messages });
  void queryClient.invalidateQueries({ queryKey: ["catch-up"] });
};

export const usePostMessage = () =>
  useMutation({ mutationFn: api.messages.post, onSuccess: refresh });

export const usePinMessage = () =>
  useMutation({
    mutationFn: ({ id, pinned }: { id: string; pinned: boolean }) =>
      api.messages.setPinned(id, pinned),
    onSuccess: refresh,
  });

export const useDeleteMessage = () =>
  useMutation({ mutationFn: api.messages.remove, onSuccess: refresh });

export const useMarkRead = () =>
  useMutation({
    mutationFn: ({ memberId, ids }: { memberId: string; ids: string[] }) =>
      api.messages.markRead(memberId, ids),
    onSuccess: refresh,
  });
