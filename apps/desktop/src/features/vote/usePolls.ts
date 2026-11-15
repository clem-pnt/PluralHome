import { useMutation, useQuery } from "@tanstack/react-query";
import { keys, queryClient } from "../../app/query";
import { api } from "../../ipc";

export const usePolls = () => useQuery({ queryKey: keys.polls, queryFn: api.polls.list });

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

export const useDeletePoll = () =>
  useMutation({ mutationFn: api.polls.remove, onSuccess: refresh });
