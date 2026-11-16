import { useQuery } from "@tanstack/react-query";
import { type ReactNode, useState } from "react";
import { keys } from "../../app/query";
import { api } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { ConnectionsPanels } from "../connections/ConnectionsView";
import { PrivacySettings, RequestsSettings } from "../friends/FriendSettings";
import { SystemPanel } from "./SystemPanel";

type PageId = "system" | "privacy" | "requests" | "connections";

type Page = {
  id: PageId;
  label: () => string;
  lead: () => string;
  /** Les réglages d'amis passent par le compte Aragonite. */
  needsAccount?: boolean;
  render: () => ReactNode;
};

/** Pages rangées par catégorie, à la façon des paramètres de Discord. */
const CATEGORIES: { title: () => string; pages: Page[] }[] = [
  {
    title: m.settings_cat_system,
    pages: [
      {
        id: "system",
        label: m.settings_system,
        lead: m.settings_system_lead,
        render: () => <SystemPanel />,
      },
    ],
  },
  {
    title: m.settings_cat_friends,
    pages: [
      {
        id: "privacy",
        label: m.settings_privacy,
        lead: m.settings_privacy_lead,
        needsAccount: true,
        render: () => <PrivacySettings />,
      },
      {
        id: "requests",
        label: m.settings_requests,
        lead: m.settings_requests_lead,
        needsAccount: true,
        render: () => <RequestsSettings />,
      },
    ],
  },
  {
    title: m.settings_cat_app,
    pages: [
      {
        id: "connections",
        label: m.settings_connections,
        lead: m.connections_lead,
        render: () => <ConnectionsPanels />,
      },
    ],
  },
];

export function SettingsView() {
  const [current, setCurrent] = useState<PageId>("system");
  const account = useQuery({ queryKey: keys.account, queryFn: api.account.get });
  const page = CATEGORIES.flatMap((c) => c.pages).find((p) => p.id === current);

  return (
    <section className="view view--settings">
      <header className="view__header">
        <div>
          <h1>{m.settings_title()}</h1>
          <p className="view__lead">{m.settings_lead()}</p>
        </div>
      </header>
      <div className="settings">
        <nav className="settings__nav" aria-label={m.settings_nav()}>
          {CATEGORIES.map((cat) => (
            <div key={cat.title()} className="settings__cat">
              <h2 className="settings__cat-title">{cat.title()}</h2>
              <ul className="settings__pages">
                {cat.pages.map((p) => (
                  <li key={p.id}>
                    <button
                      type="button"
                      className="settings__link"
                      aria-current={p.id === current ? "page" : undefined}
                      onClick={() => setCurrent(p.id)}
                    >
                      {p.label()}
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </nav>
        {page && (
          <section className="settings__section" aria-labelledby="settings-page">
            <div>
              <h2 id="settings-page">{page.label()}</h2>
              <p className="muted">{page.lead()}</p>
            </div>
            {page.needsAccount && !account.data ? (
              account.isPending ? null : (
                <section className="panel">
                  <p className="muted">{m.friends_need_account()}</p>
                </section>
              )
            ) : (
              page.render()
            )}
          </section>
        )}
      </div>
    </section>
  );
}
