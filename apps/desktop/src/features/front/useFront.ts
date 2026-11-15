import { useMutation, useQuery } from "@tanstack/react-query";
import { keys, queryClient } from "../../app/query";
import { api, type FrontStatus } from "../../ipc";
import { m } from "../../paraglide/messages.js";

export function useFrontCurrent() {
  return useQuery({ queryKey: keys.frontCurrent, queryFn: api.front.current });
}

export function useFrontChange() {
  return useMutation({
    mutationFn: api.front.change,
    onSuccess: (current) => {
      queryClient.setQueryData(keys.frontCurrent, current);
      void queryClient.invalidateQueries({ queryKey: ["front", "history"] });
      void queryClient.invalidateQueries({ queryKey: keys.frontQuick });
      void queryClient.invalidateQueries({ queryKey: ["catch-up"] });
    },
  });
}

/** Membres du switch rapide, et s'il s'agit de favoris choisis. */
export function useFrontQuick() {
  return useQuery({ queryKey: keys.frontQuick, queryFn: api.front.quick });
}

export function useSetFavorites() {
  return useMutation({
    mutationFn: api.front.setFavorites,
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: keys.frontQuick }),
  });
}

export const STATUSES: FrontStatus[] = ["front", "cofront", "coconscious"];
export const MAX_FAVORITES = 30;

export const statusLabel = (s: FrontStatus) =>
  s === "cofront"
    ? m.front_status_cofront()
    : s === "coconscious"
      ? m.front_status_coconscious()
      : m.front_status_front();
