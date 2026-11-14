import { useMutation } from "@tanstack/react-query";
import { Dialog, DialogTrigger, Heading, Modal, ModalOverlay } from "react-aria-components";
import { keys, queryClient } from "../../app/query";
import { errorMessage } from "../../i18n/errors";
import { AppError, api } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { AuthLayout } from "./AuthLayout";

/** Le coffre existe mais ne s'ouvre pas (clé disparue du trousseau, par exemple). */
export function KeyErrorScreen({ error, onRetry }: { error: unknown; onRetry: () => void }) {
  const missing = error instanceof AppError && error.code === "key_missing";
  return (
    <AuthLayout title={m.keyerror_title()} lead={missing ? m.keyerror_lead() : errorMessage(error)}>
      <div className="auth__form">
        {missing && <p className="note">{m.keyerror_note()}</p>}
        <Button block onPress={onRetry}>
          {m.keyerror_retry()}
        </Button>
      </div>
      <StartOver />
    </AuthLayout>
  );
}

function StartOver() {
  const start = useMutation({
    mutationFn: api.vault.startOver,
    onSuccess: () => queryClient.setQueryData(keys.vaultStatus, "unlocked"),
  });
  return (
    <DialogTrigger>
      <Button variant="ghost" block>
        {m.keyerror_start_over()}
      </Button>
      <ModalOverlay className="overlay" isDismissable>
        <Modal className="modal">
          <Dialog className="dialog" role="alertdialog">
            {({ close }) => (
              <>
                <Heading slot="title">{m.keyerror_start_over_title()}</Heading>
                <p className="muted">{m.keyerror_start_over_body()}</p>
                {start.isError && (
                  <p className="alert" role="alert">
                    {errorMessage(start.error)}
                  </p>
                )}
                <div className="dialog__actions">
                  <Button variant="ghost" onPress={close}>
                    {m.cancel()}
                  </Button>
                  <Button
                    variant="danger"
                    autoFocus
                    isDisabled={start.isPending}
                    onPress={() => start.mutate()}
                  >
                    {m.keyerror_start_over()}
                  </Button>
                </div>
              </>
            )}
          </Dialog>
        </Modal>
      </ModalOverlay>
    </DialogTrigger>
  );
}
