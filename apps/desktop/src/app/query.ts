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
  frontCurrent: ["front", "current"] as const,
  frontHistory: (day: number) => ["front", "history", day] as const,
  messages: ["messages"] as const,
  catchUp: (memberId: string) => ["catch-up", memberId] as const,
  account: ["account"] as const,
  pkStatus: ["pluralkit", "status"] as const,
  safetyPlan: ["safety-plan"] as const,
  journal: (author: string | null) => ["journal", author] as const,
};
