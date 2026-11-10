import { useMutation } from "@tanstack/react-query";
import { useState } from "react";
import { DialogTrigger, Form } from "react-aria-components";
import { keys, queryClient } from "../../app/query";
import { errorMessage } from "../../i18n/errors";
import { api } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { TextField } from "../../ui/TextField";
import { RestoreDialog } from "../backup/RestoreDialog";
import { AuthLayout } from "./AuthLayout";

const MIN_CHARS = 8;

export function SetupScreen() {
  const [passphrase, setPassphrase] = useState("");
  const [confirm, setConfirm] = useState("");
  const create = useMutation({
    mutationFn: api.vault.create,
    onSuccess: () => queryClient.setQueryData(keys.vaultStatus, "unlocked"),
  });

  return (
    <AuthLayout title={m.setup_title()} lead={m.setup_lead()}>
      <Form
        className="auth__form"
        onSubmit={(e) => {
          e.preventDefault();
          create.mutate(passphrase);
        }}
      >
        <TextField
          label={m.setup_passphrase()}
          hint={m.setup_passphrase_hint()}
          type="password"
          autoComplete="new-password"
          autoFocus
          isRequired
          minLength={MIN_CHARS}
          value={passphrase}
          onChange={setPassphrase}
        />
        <TextField
          label={m.setup_confirm()}
          type="password"
          autoComplete="new-password"
          isRequired
          value={confirm}
          onChange={setConfirm}
          validate={(v) => (v && v !== passphrase ? m.setup_mismatch() : null)}
        />
        <p className="note">{m.setup_warning()}</p>
        {create.isError && (
          <p className="alert" role="alert">
            {errorMessage(create.error)}
          </p>
        )}
        <Button type="submit" block isDisabled={create.isPending}>
          {create.isPending ? m.working() : m.setup_submit()}
        </Button>
      </Form>
      <DialogTrigger>
        <Button variant="ghost" block>
          {m.backup_have_one()}
        </Button>
        <RestoreDialog />
      </DialogTrigger>
    </AuthLayout>
  );
}
