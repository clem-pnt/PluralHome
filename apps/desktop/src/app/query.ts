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
  membersArchived: ["members", "archived"] as const,
  roles: ["roles"] as const,
  folders: ["folders"] as const,
  frontCurrent: ["front", "current"] as const,
  frontHistory: (day: number) => ["front", "history", day] as const,
  frontRange: (from: number, to: number) => ["front", "history", "range", from, to] as const,
  frontQuick: ["front", "quick"] as const,
  messages: ["messages"] as const,
  polls: ["polls"] as const,
  catchUp: (memberId: string) => ["catch-up", memberId] as const,
  account: ["account"] as const,
  cloudStatus: ["cloud", "status"] as const,
  cloudDevices: ["cloud", "devices"] as const,
  pkStatus: ["pluralkit", "status"] as const,
  safetyPlan: ["safety-plan"] as const,
  newcomerPlan: ["newcomer-plan"] as const,
  trusted: ["trusted"] as const,
  journal: (author: string | null) => ["journal", author] as const,
  groups: ["groups"] as const,
  system: ["system"] as const,
  friends: ["friends"] as const,
  friendsList: ["friends", "list"] as const,
  friendsRequests: ["friends", "requests"] as const,
  friendsInvites: ["friends", "invites"] as const,
  friendsPrivacy: ["friends", "privacy"] as const,
  friendsSettings: ["friends", "settings"] as const,
  friendsBlocks: ["friends", "blocks"] as const,
};
