import { useMutation } from "@tanstack/react-query";
import { useState } from "react";
import { Form } from "react-aria-components";
import { keys, queryClient } from "../../app/query";
import { errorMessage } from "../../i18n/errors";
import { api } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { TextField } from "../../ui/TextField";
import { AuthLayout } from "./AuthLayout";

export function UnlockScreen() {
  const [passphrase, setPassphrase] = useState("");
  const unlock = useMutation({
    mutationFn: api.vault.unlock,
    onSuccess: () => queryClient.setQueryData(keys.vaultStatus, "unlocked"),
    onError: () => setPassphrase(""),
  });

  return (
    <AuthLayout title={m.unlock_title()} lead={m.unlock_lead()}>
      <Form
        className="auth__form"
        onSubmit={(e) => {
          e.preventDefault();
          unlock.mutate(passphrase);
        }}
      >
        <TextField
          label={m.setup_passphrase()}
          type="password"
          autoComplete="current-password"
          autoFocus
          isRequired
          value={passphrase}
          onChange={setPassphrase}
        />
        {unlock.isError && (
          <p className="alert" role="alert">
            {errorMessage(unlock.error)}
          </p>
        )}
        <Button type="submit" block isDisabled={unlock.isPending}>
          {unlock.isPending ? m.working() : m.unlock_submit()}
        </Button>
      </Form>
    </AuthLayout>
  );
}
