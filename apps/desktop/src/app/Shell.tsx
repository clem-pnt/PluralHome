import { useState } from "react";
import { Button } from "react-aria-components";
import { BackupView } from "../features/backup/BackupView";
import { FrontView } from "../features/front/FrontView";
import { MembersView } from "../features/members/MembersView";
import { MessagesView } from "../features/messages/MessagesView";
import { m } from "../paraglide/messages.js";
import { Logo } from "../ui/Logo";

const VIEWS = {
  front: { label: m.nav_front, Component: FrontView },
  messages: { label: m.nav_messages, Component: MessagesView },
  members: { label: m.nav_members, Component: MembersView },
  backup: { label: m.nav_backup, Component: BackupView },
} as const;
type View = keyof typeof VIEWS;

export function Shell({ onLock }: { onLock: () => void }) {
  const [view, setView] = useState<View>("front");
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
