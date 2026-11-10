import { useMutation } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { Dialog, Form, Heading, Modal, ModalOverlay } from "react-aria-components";
import { keys, queryClient } from "../../app/query";
import { errorMessage } from "../../i18n/errors";
import { api } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { TextField } from "../../ui/TextField";

export const BACKUP_FILTER = { name: m.backup_file_label(), extensions: ["phbackup"] };

export function RestoreDialog() {
  return (
    <ModalOverlay className="overlay" isDismissable>
      <Modal className="modal">
        <Dialog className="dialog">{({ close }) => <RestoreForm onDone={close} />}</Dialog>
      </Modal>
    </ModalOverlay>
  );
}

function RestoreForm({ onDone }: { onDone: () => void }) {
  const [path, setPath] = useState<string | null>(null);
  const [passphrase, setPassphrase] = useState("");
  const restore = useMutation({
    mutationFn: ({ file, pass }: { file: string; pass: string }) => api.backup.restore(file, pass),
    onSuccess: () => {
      // Tout ce qui était en cache décrit l'ancien coffre.
      queryClient.clear();
      queryClient.setQueryData(keys.vaultStatus, "unlocked");
      onDone();
    },
    onError: () => setPassphrase(""),
  });

  const pick = async () => {
    const file = await open({ multiple: false, directory: false, filters: [BACKUP_FILTER] });
    if (typeof file === "string") setPath(file);
  };

  return (
    <Form
      className="dialog"
      style={{ padding: 0 }}
      onSubmit={(e) => {
        e.preventDefault();
        if (path) restore.mutate({ file: path, pass: passphrase });
      }}
    >
      <div>
        <Heading slot="title">{m.backup_restore()}</Heading>
        <p className="muted">{m.backup_restore_body()}</p>
      </div>
      <div className="file-pick">
        <Button variant="ghost" onPress={() => void pick()}>
          {m.backup_restore_pick()}
        </Button>
        {path && (
          <span className="file-pick__name" title={path}>
            {path.split(/[\\/]/).at(-1)}
          </span>
        )}
      </div>
      {path && (
        <TextField
          label={m.backup_restore_passphrase()}
          type="password"
          autoComplete="off"
          autoFocus
          isRequired
          value={passphrase}
          onChange={setPassphrase}
        />
      )}
      {restore.isError && (
        <p className="alert" role="alert">
          {errorMessage(restore.error)}
        </p>
      )}
      <div className="dialog__actions">
        <Button variant="ghost" slot="close">
          {m.cancel()}
        </Button>
        <Button type="submit" isDisabled={!path || !passphrase || restore.isPending}>
          {restore.isPending ? m.working() : m.backup_restore_confirm()}
        </Button>
      </div>
    </Form>
  );
}
