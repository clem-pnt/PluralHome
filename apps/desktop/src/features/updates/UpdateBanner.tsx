import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { useEffect, useState } from "react";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";

const CHECK_EVERY_MS = 6 * 60 * 60 * 1000;

type State =
  | { kind: "idle" }
  | { kind: "downloading"; version: string }
  | { kind: "ready"; version: string };

/**
 * Mises à jour poussées : vérification au démarrage puis toutes les 6 h,
 * téléchargement et installation en arrière-plan (signature vérifiée par le plugin),
 * puis proposition de redémarrer. Aucune donnée n'est envoyée au serveur.
 */
export function UpdateBanner() {
  const [state, setState] = useState<State>({ kind: "idle" });

  useEffect(() => {
    if (import.meta.env.DEV) return;
    let busy = false;
    const run = async () => {
      if (busy) return;
      busy = true;
      try {
        const update: Update | null = await check();
        if (!update) return;
        setState({ kind: "downloading", version: update.version });
        await update.downloadAndInstall();
        setState({ kind: "ready", version: update.version });
      } catch (e) {
        console.warn("mise à jour impossible", e);
        setState({ kind: "idle" });
      } finally {
        busy = false;
      }
    };
    void run();
    const id = window.setInterval(run, CHECK_EVERY_MS);
    return () => window.clearInterval(id);
  }, []);

  if (state.kind === "idle") return null;
  return (
    <div className="update" role="status">
      {state.kind === "downloading" ? (
        <span>{m.update_downloading({ version: state.version })}</span>
      ) : (
        <>
          <span>{m.update_ready({ version: state.version })}</span>
          <Button variant="accent" onPress={() => void relaunch()}>
            {m.update_restart()}
          </Button>
        </>
      )}
    </div>
  );
}
