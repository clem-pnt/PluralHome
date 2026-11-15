import { useState } from "react";
import { Dialog, Heading, Modal, ModalOverlay } from "react-aria-components";
import { errorMessage } from "../../i18n/errors";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { MemberPicker } from "../members/MemberPicker";
import { useMembers } from "../members/useMembers";
import { MAX_FAVORITES, useSetFavorites } from "./useFront";

export function FavoritesDialog({ initial }: { initial: string[] }) {
  return (
    <ModalOverlay className="overlay" isDismissable>
      <Modal className="modal modal--wide">
        <Dialog className="dialog">
          {({ close }) => <Favorites initial={initial} onDone={close} />}
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}

function Favorites({ initial, onDone }: { initial: string[]; onDone: () => void }) {
  const members = useMembers();
  const save = useSetFavorites();
  const [value, setValue] = useState(initial);
  const tooMany = value.length > MAX_FAVORITES;

  return (
    <>
      <div>
        <Heading slot="title">{m.front_favorites_title()}</Heading>
        <p className="muted">{m.front_favorites_lead({ max: MAX_FAVORITES })}</p>
      </div>
      <MemberPicker
        label={m.front_favorites_title()}
        members={members.list}
        value={value}
        onChange={setValue}
        autoFocus
      />
      {tooMany && (
        <p className="alert" role="alert">
          {m.front_favorites_too_many({ max: MAX_FAVORITES })}
        </p>
      )}
      {save.isError && (
        <p className="alert" role="alert">
          {errorMessage(save.error)}
        </p>
      )}
      <div className="dialog__actions">
        <Button
          variant="ghost"
          isDisabled={save.isPending}
          onPress={() => save.mutate([], { onSuccess: onDone })}
        >
          {m.front_favorites_reset()}
        </Button>
        <span style={{ flex: 1 }} />
        <Button variant="ghost" slot="close">
          {m.cancel()}
        </Button>
        <Button
          isDisabled={save.isPending || tooMany}
          onPress={() => save.mutate(value, { onSuccess: onDone })}
        >
          {m.front_save()}
        </Button>
      </div>
    </>
  );
}
