import { useMutation, useQuery } from "@tanstack/react-query";
import { keys, queryClient } from "../../app/query";
import { api } from "../../ipc";

export function useFrontCurrent() {
  return useQuery({ queryKey: keys.frontCurrent, queryFn: api.front.current });
}

export function useFrontChange() {
  return useMutation({
    mutationFn: api.front.change,
    onSuccess: (current) => {
      queryClient.setQueryData(keys.frontCurrent, current);
      void queryClient.invalidateQueries({ queryKey: ["front", "history"] });
      void queryClient.invalidateQueries({ queryKey: ["catch-up"] });
    },
  });
}
