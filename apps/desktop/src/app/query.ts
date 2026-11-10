import { QueryClient } from "@tanstack/react-query";

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: { staleTime: Number.POSITIVE_INFINITY, retry: false, refetchOnWindowFocus: false },
    mutations: { retry: false },
  },
});

export const keys = {
  vaultStatus: ["vault", "status"] as const,
  members: ["members"] as const,
};
