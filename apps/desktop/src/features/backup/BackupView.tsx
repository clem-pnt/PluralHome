import { useMutation } from "@tanstack/react-query";
import { save } from "@tauri-apps/plugin-dialog";
import { DialogTrigger } from "react-aria-components";
import { errorMessage } from "../../i18n/errors";
import { api } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { BACKUP_FILTER, RestoreDialog } from "./RestoreDialog";

const today = () => new Date().toISOString().slice(0, 10);

export function BackupView() {
  const exporter = useMutation({
    mutationFn: async () => {
      const path = await save({
        defaultPath: `pluralhome-${today()}.phbackup`,
        filters: [BACKUP_FILTER],
      });
      if (!path) return false;
      await api.backup.export(path.endsWith(".phbackup") ? path : `${path}.phbackup`);
      return true;
    },
  });

  return (
    <section className="view">
      <header className="view__header">
        <div>
          <h1>{m.backup_title()}</h1>
          <p className="view__lead">{m.backup_lead()}</p>
        </div>
      </header>

      <div className="panel">
        <h2 className="panel__title">{m.backup_export()}</h2>
        <p className="muted">{m.backup_export_body()}</p>
        <p className="note">{m.backup_advice()}</p>
        <div>
          <Button isDisabled={exporter.isPending} onPress={() => exporter.mutate()}>
            {exporter.isPending ? m.working() : m.backup_export()}
          </Button>
        </div>
        {exporter.data === true && (
          <p className="muted" role="status">
            {m.backup_export_done()}
          </p>
        )}
        {exporter.isError && (
          <p className="alert" role="alert">
            {errorMessage(exporter.error)}
          </p>
        )}
      </div>

      <div className="panel">
        <h2 className="panel__title">{m.backup_restore()}</h2>
        <p className="muted">{m.backup_restore_body()}</p>
        <div>
          <DialogTrigger>
            <Button variant="ghost">{m.backup_restore()}</Button>
            <RestoreDialog />
          </DialogTrigger>
        </div>
      </div>
    </section>
  );
}
