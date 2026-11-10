import { Button } from "react-aria-components";
import { MembersView } from "../features/members/MembersView";
import { m } from "../paraglide/messages.js";
import { Logo } from "../ui/Logo";

export function Shell({ onLock }: { onLock: () => void }) {
  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="sidebar__brand">
          <Logo size={34} />
          <span>{m.app_name()}</span>
        </div>
        <nav className="sidebar__nav" aria-label={m.app_name()}>
          <a className="navlink" aria-current="page" href="#members">
            {m.nav_members()}
          </a>
        </nav>
        <Button className="navlink navlink--quiet" onPress={onLock}>
          {m.nav_lock()}
        </Button>
      </aside>
      <main className="main" id="members">
        <MembersView />
      </main>
    </div>
  );
}
