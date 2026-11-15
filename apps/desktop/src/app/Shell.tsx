import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { Button } from "react-aria-components";
import { FriendsView } from "../features/friends/FriendsView";
import { FrontView } from "../features/front/FrontView";
import { useFrontCurrent } from "../features/front/useFront";
import { GroundingView } from "../features/grounding/GroundingView";
import { JournalView } from "../features/journal/JournalView";
import { MembersView } from "../features/members/MembersView";
import { MessagesView } from "../features/messages/MessagesView";
import { presentIds } from "../features/messages/people";
import { SettingsView } from "../features/settings/SettingsView";
import { pollAlerts } from "../features/vote/PollNotice";
import { usePolls } from "../features/vote/usePolls";
import { VoteView } from "../features/vote/VoteView";
import { api } from "../ipc";
import { m } from "../paraglide/messages.js";
import { Logo } from "../ui/Logo";
import { NavContext, type ViewKey } from "./nav";
import { keys, queryClient } from "./query";
import { useKind } from "./useKind";

const VIEWS = {
  front: { label: m.nav_front, Component: FrontView },
  messages: { label: m.nav_messages, Component: MessagesView },
  friends: { label: m.nav_friends, Component: FriendsView },
  vote: { label: m.nav_vote, Component: VoteView },
  journal: { label: m.nav_journal, Component: JournalView },
  members: { label: m.nav_members, Component: MembersView },
  grounding: { label: m.nav_grounding, Component: GroundingView },
  settings: { label: m.nav_settings, Component: SettingsView },
} as const satisfies Record<ViewKey, unknown>;

/** Émis par la boucle de synchro (src-tauri/src/remote.rs) quand des données ont changé. */
const EVENT_SYNCED = "pluralkit://synced";
/** Émis quand la synchro chiffrée a reçu des changements d'un autre appareil. */
const EVENT_CLOUD = "cloud://synced";
/** Émis quand les amis, leurs fiches ou leurs messages ont changé. */
const EVENT_FRIENDS = "friends://synced";

function useRemoteSync() {
  useEffect(() => {
    const off = listen(EVENT_SYNCED, () => {
      for (const queryKey of [
        keys.members,
        keys.roles,
        keys.folders,
        ["front"],
        ["catch-up"],
        keys.pkStatus,
      ]) {
        void queryClient.invalidateQueries({ queryKey });
      }
    });
    const offCloud = listen(EVENT_CLOUD, () => {
      void queryClient.invalidateQueries();
    });
    const offFriends = listen(EVENT_FRIENDS, () => {
      for (const queryKey of [keys.friends, keys.messages, keys.groups, ["catch-up"]]) {
        void queryClient.invalidateQueries({ queryKey });
      }
    });
    return () => {
      void offFriends.then((unlisten) => unlisten());
      void off.then((unlisten) => unlisten());
      void offCloud.then((unlisten) => unlisten());
    };
  }, []);
}

/** Au lancement, relit le compte Aragonite (nom, photo) s'il y en a un. Silencieux hors ligne. */
function useAccountRefresh() {
  useEffect(() => {
    void api.account
      .get()
      .then((a) => (a ? api.account.refresh() : null))
      .then((a) => {
        if (a) queryClient.setQueryData(keys.account, a);
      })
      // Session refusée par le SSO : le cœur pose le drapeau de déconnexion.
      .catch(() => queryClient.invalidateQueries({ queryKey: keys.signedOut }));
  }, []);
}

type View = ViewKey;

/** Pastille de la vue Vote : votes en attente de réponse et résultats à voir. */
function useVoteBadge(enabled: boolean): number {
  const polls = usePolls();
  const current = useFrontCurrent();
  if (!enabled) return 0;
  const { pending, ended } = pollAlerts(polls.data ?? [], presentIds(current.data));
  return pending.length + ended.length;
}

/** Sans alters, ces vues n'ont pas de sens : masquées, pas supprimées. */
const SYSTEM_ONLY: readonly View[] = ["front", "members", "vote"];

export function Shell() {
  const kind = useKind();
  const views = (Object.keys(VIEWS) as View[]).filter(
    (v) => kind === "system" || !SYSTEM_ONLY.includes(v),
  );
  const [chosen, setView] = useState<View | null>(null);
  const fallback: View = kind === "system" ? "front" : "journal";
  const view = chosen && views.includes(chosen) ? chosen : fallback;
  useRemoteSync();
  useAccountRefresh();
  const voteBadge = useVoteBadge(views.includes("vote"));
  const { Component } = VIEWS[view];

  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="sidebar__brand">
          <Logo size={34} />
          <span>{m.app_name()}</span>
        </div>
        <nav className="sidebar__nav" aria-label={m.app_name()}>
          {views.map((key) => (
            <Button
              key={key}
              className="navlink"
              aria-current={view === key ? "page" : false}
              onPress={() => setView(key)}
            >
              {VIEWS[key].label()}
              {key === "vote" && voteBadge > 0 && (
                <span
                  className="navlink__badge"
                  role="img"
                  aria-label={m.vote_notice_badge({ count: voteBadge })}
                >
                  {voteBadge}
                </span>
              )}
            </Button>
          ))}
        </nav>
      </aside>
      <main className="main">
        <NavContext.Provider value={setView}>
          <Component />
        </NavContext.Provider>
      </main>
    </div>
  );
}
