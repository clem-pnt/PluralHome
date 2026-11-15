import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { Button } from "react-aria-components";
import { FrontView } from "../features/front/FrontView";
import { GroundingView } from "../features/grounding/GroundingView";
import { JournalView } from "../features/journal/JournalView";
import { MembersView } from "../features/members/MembersView";
import { MessagesView } from "../features/messages/MessagesView";
import { SettingsView } from "../features/settings/SettingsView";
import { VoteView } from "../features/vote/VoteView";
import { m } from "../paraglide/messages.js";
import { Logo } from "../ui/Logo";
import { keys, queryClient } from "./query";

const VIEWS = {
  front: { label: m.nav_front, Component: FrontView },
  messages: { label: m.nav_messages, Component: MessagesView },
  vote: { label: m.nav_vote, Component: VoteView },
  journal: { label: m.nav_journal, Component: JournalView },
  members: { label: m.nav_members, Component: MembersView },
  grounding: { label: m.nav_grounding, Component: GroundingView },
  settings: { label: m.nav_settings, Component: SettingsView },
} as const;

/** Émis par la boucle de synchro (src-tauri/src/remote.rs) quand des données ont changé. */
const EVENT_SYNCED = "pluralkit://synced";
/** Émis quand la synchro chiffrée a reçu des changements d'un autre appareil. */
const EVENT_CLOUD = "cloud://synced";

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
    return () => {
      void off.then((unlisten) => unlisten());
      void offCloud.then((unlisten) => unlisten());
    };
  }, []);
}
type View = keyof typeof VIEWS;

export function Shell() {
  const [view, setView] = useState<View>("front");
  useRemoteSync();
  const { Component } = VIEWS[view];

  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="sidebar__brand">
          <Logo size={34} />
          <span>{m.app_name()}</span>
        </div>
        <nav className="sidebar__nav" aria-label={m.app_name()}>
          {(Object.keys(VIEWS) as View[]).map((key) => (
            <Button
              key={key}
              className="navlink"
              aria-current={view === key ? "page" : false}
              onPress={() => setView(key)}
            >
              {VIEWS[key].label()}
            </Button>
          ))}
        </nav>
      </aside>
      <main className="main">
        <Component />
      </main>
    </div>
  );
}
