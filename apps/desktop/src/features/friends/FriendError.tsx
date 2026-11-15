import { useMutation } from "@tanstack/react-query";
import { Dialog, DialogTrigger, Heading, Modal, ModalOverlay } from "react-aria-components";
import { errorMessage } from "../../i18n/errors";
import { api } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { isIdentityConflict, refreshFriends } from "./useFriends";

/**
 * Erreur d'une action d'amis. Si un autre appareil du compte porte une autre
 * identité, propose de la remplacer (ce qui coupe toutes les amitiés existantes).
 */
export function FriendError({ error }: { error: unknown }) {
  if (!error) return null;
  if (!isIdentityConflict(error)) {
    return (
      <p className="alert" role="alert">
        {errorMessage(error)}
      </p>
    );
  }
  return (
    <div className="alert" role="alert">
      <p>{m.friends_conflict_body()}</p>
      <TakeOverButton />
    </div>
  );
}

function TakeOverButton() {
  const take = useMutation({ mutationFn: api.friends.takeOver, onSuccess: refreshFriends });
  return (
    <DialogTrigger>
      <Button variant="danger">{m.friends_take_over()}</Button>
      <ModalOverlay className="overlay" isDismissable>
        <Modal className="modal">
          <Dialog className="dialog" role="alertdialog">
            {({ close }) => (
              <>
                <Heading slot="title">{m.friends_take_over_title()}</Heading>
                <p className="muted">{m.friends_take_over_body()}</p>
                {take.isError && (
                  <p className="alert" role="alert">
                    {errorMessage(take.error)}
                  </p>
                )}
                <div className="dialog__actions">
                  <Button variant="ghost" onPress={close}>
                    {m.cancel()}
                  </Button>
                  <Button
                    variant="danger"
                    isDisabled={take.isPending}
                    onPress={() => take.mutate(undefined, { onSuccess: close })}
                  >
                    {m.friends_take_over()}
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

/** Dialogue de confirmation simple pour une action irréversible. */
export function ConfirmButton({
  label,
  title,
  body,
  confirm,
  variant = "ghost",
  onConfirm,
  pending,
  error,
}: {
  label: string;
  title: string;
  body: string;
  confirm: string;
  variant?: "ghost" | "danger";
  onConfirm: (close: () => void) => void;
  pending: boolean;
  error: unknown;
}) {
  return (
    <DialogTrigger>
      <Button variant={variant}>{label}</Button>
      <ModalOverlay className="overlay" isDismissable>
        <Modal className="modal">
          <Dialog className="dialog" role="alertdialog">
            {({ close }) => (
              <>
                <Heading slot="title">{title}</Heading>
                <p className="muted">{body}</p>
                <FriendError error={error} />
                <div className="dialog__actions">
                  <Button variant="ghost" onPress={close}>
                    {m.cancel()}
                  </Button>
                  <Button variant="danger" isDisabled={pending} onPress={() => onConfirm(close)}>
                    {confirm}
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
