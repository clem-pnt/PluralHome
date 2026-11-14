import { useMutation } from "@tanstack/react-query";
import { save } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { DialogTrigger, Form } from "react-aria-components";
import { errorMessage } from "../../i18n/errors";
import { api } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { TextField } from "../../ui/TextField";
import { BACKUP_FILTER, RestoreDialog } from "./RestoreDialog";

const today = () => new Date().toISOString().slice(0, 10);

export function BackupView() {
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const exporter = useMutation({
    mutationFn: async (password: string) => {
      const path = await save({
        defaultPath: `pluralhome-${today()}.phbackup`,
        filters: [BACKUP_FILTER],
      });
      if (!path) return false;
      await api.backup.export(path.endsWith(".phbackup") ? path : `${path}.phbackup`, password);
      return true;
    },
    onSuccess: (done) => {
      if (done) {
        setPassword("");
        setConfirm("");
      }
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
        <Form
          className="auth__form"
          onSubmit={(e) => {
            e.preventDefault();
            exporter.mutate(password);
          }}
        >
          <TextField
            label={m.backup_password_new()}
            hint={m.backup_password_hint()}
            type="password"
            autoComplete="new-password"
            isRequired
            minLength={8}
            value={password}
            onChange={setPassword}
          />
          <TextField
            label={m.backup_password_confirm()}
            type="password"
            autoComplete="new-password"
            isRequired
            value={confirm}
            onChange={setConfirm}
            validate={(v) => (v && v !== password ? m.backup_password_mismatch() : null)}
          />
          <div>
            <Button type="submit" isDisabled={exporter.isPending}>
              {exporter.isPending ? m.working() : m.backup_export()}
            </Button>
          </div>
        </Form>
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
