import { m } from "../../paraglide/messages.js";
import { ConnectionsPanels } from "../connections/ConnectionsView";

export function SettingsView() {
  return (
    <section className="view">
      <header className="view__header">
        <div>
          <h1>{m.settings_title()}</h1>
          <p className="view__lead">{m.settings_lead()}</p>
        </div>
      </header>
      <section className="settings__section" aria-labelledby="settings-connections">
        <div>
          <h2 id="settings-connections">{m.settings_connections()}</h2>
          <p className="muted">{m.connections_lead()}</p>
        </div>
        <ConnectionsPanels />
      </section>
    </section>
  );
}
