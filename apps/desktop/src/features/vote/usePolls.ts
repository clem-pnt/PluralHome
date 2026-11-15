import { useMutation, useQuery } from "@tanstack/react-query";
import { useEffect } from "react";
import { keys, queryClient } from "../../app/query";
import { api, type Poll } from "../../ipc";

export const usePolls = () => {
  const q = useQuery({ queryKey: keys.polls, queryFn: api.polls.list });
  // Relit la liste à la prochaine échéance : le vote passe en « terminé » tout seul.
  const next = Math.min(
    ...(q.data ?? []).flatMap((p) => (p.closedAt == null && p.endsAt != null ? [p.endsAt] : [])),
  );
  useEffect(() => {
    if (!Number.isFinite(next)) return;
    // setTimeout est limité à ~24 jours : au-delà, on revérifie plus tard.
    const wait = Math.min(Math.max(0, next - Date.now()) + 500, 2 ** 31 - 1);
    const t = setTimeout(refresh, wait);
    return () => clearTimeout(t);
  }, [next]);
  return q;
};

/** Vote ouvert : ni clos, ni arrivé à échéance. */
export const isOpen = (p: Poll) => p.closedAt == null;

const refresh = () => void queryClient.invalidateQueries({ queryKey: keys.polls });

export const useCreatePoll = () =>
  useMutation({ mutationFn: api.polls.create, onSuccess: refresh });

export const useVote = () =>
  useMutation({
    mutationFn: ({ id, voters, options }: { id: string; voters: string[]; options: string[] }) =>
      api.polls.vote(id, voters, options),
    onSuccess: refresh,
  });

export const useSetPollClosed = () =>
  useMutation({
    mutationFn: ({ id, closed }: { id: string; closed: boolean }) =>
      api.polls.setClosed(id, closed),
    onSuccess: refresh,
  });

export const useMarkResultSeen = () =>
  useMutation({ mutationFn: api.polls.markResultSeen, onSuccess: refresh });

export const useDeletePoll = () =>
  useMutation({ mutationFn: api.polls.remove, onSuccess: refresh });
