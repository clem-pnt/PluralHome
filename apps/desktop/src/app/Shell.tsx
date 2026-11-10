import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { Button } from "react-aria-components";
import { BackupView } from "../features/backup/BackupView";
import { ConnectionsView } from "../features/connections/ConnectionsView";
import { FrontView } from "../features/front/FrontView";
import { MembersView } from "../features/members/MembersView";
import { MessagesView } from "../features/messages/MessagesView";
import { m } from "../paraglide/messages.js";
import { Logo } from "../ui/Logo";
import { keys, queryClient } from "./query";

const VIEWS = {
  front: { label: m.nav_front, Component: FrontView },
  messages: { label: m.nav_messages, Component: MessagesView },
  members: { label: m.nav_members, Component: MembersView },
  backup: { label: m.nav_backup, Component: BackupView },
  connections: { label: m.nav_connections, Component: ConnectionsView },
} as const;

/** Émis par la boucle de synchro (src-tauri/src/remote.rs) quand des données ont changé. */
const EVENT_SYNCED = "pluralkit://synced";

function useRemoteSync() {
  useEffect(() => {
    const off = listen(EVENT_SYNCED, () => {
      for (const queryKey of [keys.members, ["front"], ["catch-up"], keys.pkStatus]) {
        void queryClient.invalidateQueries({ queryKey });
      }
    });
    return () => {
      void off.then((unlisten) => unlisten());
    };
  }, []);
}
type View = keyof typeof VIEWS;

export function Shell({ onLock }: { onLock: () => void }) {
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
        <Button className="navlink navlink--quiet" onPress={onLock}>
          {m.nav_lock()}
        </Button>
      </aside>
      <main className="main">
        <Component />
      </main>
    </div>
  );
}
