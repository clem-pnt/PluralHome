import { useInfiniteQuery, useMutation } from "@tanstack/react-query";
import { keys, queryClient } from "../../app/query";
import { api, type NewMessage } from "../../ipc";

const PAGE = 50;

/** `unreadOnly` : seulement ce qui n'a pas encore été lu (les épinglés restent). */
export function useMessageBoard(unreadOnly: boolean) {
  return useInfiniteQuery({
    queryKey: [...keys.messages, unreadOnly ? "unread" : "all"],
    queryFn: ({ pageParam }) => api.messages.list(pageParam, PAGE, unreadOnly),
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

export const useUpdateMessage = () =>
  useMutation({
    mutationFn: ({ id, input }: { id: string; input: NewMessage }) =>
      api.messages.update(id, input),
    onSuccess: refresh,
  });

export const usePinMessage = () =>
  useMutation({
    mutationFn: ({ id, pinned }: { id: string; pinned: boolean }) =>
      api.messages.setPinned(id, pinned),
    onSuccess: refresh,
  });

export const useDeleteMessage = () =>
  useMutation({ mutationFn: api.messages.remove, onSuccess: refresh });

/** Lu par ces membres, depuis le tableau. */
export const useMarkReadBy = () =>
  useMutation({
    mutationFn: ({ id, members }: { id: string; members: string[] }) =>
      api.messages.markReadBy(id, members),
    onSuccess: refresh,
  });

/** « J'ai lu » du rattrapage : messages et passages au front. */
export const useMarkRead = () =>
  useMutation({
    mutationFn: ({ memberId, ids }: { memberId: string; ids: string[] }) =>
      api.messages.markRead(memberId, ids),
    onSuccess: refresh,
  });
