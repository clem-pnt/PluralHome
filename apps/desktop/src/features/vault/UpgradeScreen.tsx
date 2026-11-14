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

/** Coffre d'avant 0.9 : la phrase secrète est demandée une dernière fois. */
export function UpgradeScreen() {
  const [passphrase, setPassphrase] = useState("");
  const unlock = useMutation({
    mutationFn: api.vault.upgrade,
    onSuccess: () => queryClient.setQueryData(keys.vaultStatus, "unlocked"),
    onError: () => setPassphrase(""),
  });

  return (
    <AuthLayout title={m.upgrade_title()} lead={m.upgrade_lead()}>
      <Form
        className="auth__form"
        onSubmit={(e) => {
          e.preventDefault();
          unlock.mutate(passphrase);
        }}
      >
        <TextField
          label={m.upgrade_passphrase()}
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
          {unlock.isPending ? m.working() : m.upgrade_submit()}
        </Button>
      </Form>
    </AuthLayout>
  );
}
