import { createContext, useContext } from "react";

/** Vues de la barre latérale (voir Shell.tsx). */
export type ViewKey =
  | "front"
  | "messages"
  | "friends"
  | "vote"
  | "journal"
  | "members"
  | "grounding"
  | "settings";

/** Changer de vue depuis une page (ex. « Aller voter » sur la page Front). */
export const NavContext = createContext<(view: ViewKey) => void>(() => {});
export const useNavigate = () => useContext(NavContext);
