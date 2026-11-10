import { Dialog, DialogTrigger, Heading, Modal, ModalOverlay } from "react-aria-components";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { Grounding } from "./Grounding";

/** Accès à l'ancrage depuis les écrans de déverrouillage, sans la phrase secrète. */
export function GroundingDialog() {
  return (
    <DialogTrigger>
      <Button variant="ghost">{m.grounding_open()}</Button>
      <ModalOverlay className="overlay" isDismissable>
        <Modal className="modal modal--wide">
          <Dialog className="dialog">
            {({ close }) => (
              <>
                <Heading slot="title">{m.grounding_title()}</Heading>
                <Grounding withPlan={false} />
                <div className="dialog__actions">
                  <Button variant="ghost" onPress={close}>
                    {m.close()}
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
