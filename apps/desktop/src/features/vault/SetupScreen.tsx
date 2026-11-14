import { useMutation } from "@tanstack/react-query";
import { keys, queryClient } from "../../app/query";
import { errorMessage } from "../../i18n/errors";
import { api } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { AuthLayout } from "./AuthLayout";

export function SetupScreen() {
  const create = useMutation({
    mutationFn: api.vault.create,
    onSuccess: () => queryClient.setQueryData(keys.vaultStatus, "unlocked"),
  });

  return (
    <AuthLayout title={m.setup_title()} lead={m.setup_lead()}>
      <div className="auth__form">
        <p className="note">{m.setup_note()}</p>
        {create.isError && (
          <p className="alert" role="alert">
            {errorMessage(create.error)}
          </p>
        )}
        <Button block isDisabled={create.isPending} onPress={() => create.mutate()}>
          {create.isPending ? m.working() : m.setup_submit()}
        </Button>
      </div>
    </AuthLayout>
  );
}
