import { useSystem } from "../features/messages/useGroups";
import type { SystemKind } from "../ipc";

/** Singlet ou système ; les bases d'avant ce choix sont des systèmes. */
export function useKind(): SystemKind {
  return useSystem().data?.kind ?? "system";
}
