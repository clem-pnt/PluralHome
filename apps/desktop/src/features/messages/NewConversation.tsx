import { useState } from "react";
import { Button as AriaButton, Dialog, Heading, Modal, ModalOverlay } from "react-aria-components";
import type { ChatGroup, Member } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { Icon } from "../../ui/Icon";
import { friendName, useFriends } from "../friends/useFriends";
import { GroupForm } from "./GroupForm";
import { type PickSystem, SELF, type SystemPick, SystemsPicker } from "./SystemsPicker";
import { useSystem } from "./useGroups";
import { systemLabel } from "./who";

type Step = "choose" | "discussion" | "group";

/**
 * Discussion à ouvrir. Sans ami : `from` et `to` sont nos membres (`to` vide :
 * tout le système). Avec un ami : `from` vide signe au nom de tout notre
 * système, `to` liste les membres de sa fiche (vide : tout son système).
 */
export interface Draft {
  friendId: string | null;
  from: string[];
  to: string[];
}

/** Notre système, pour les sélecteurs. */
export function useSelfSystem(members: Member[]): PickSystem {
  const system = useSystem();
  return {
    id: SELF,
    name: systemLabel(system.data),
    avatar: system.data?.avatar ?? null,
    members,
  };
}

/** Amis à qui l'on peut écrire, sous forme de systèmes à choisir. */
export function useFriendSystems(): PickSystem[] {
  const friends = useFriends();
  return friends.list
    .filter((f) => f.acceptsMessages)
    .map((f) => ({
      id: f.id,
      name: friendName(f),
      avatar: f.avatar ?? null,
      members: f.members,
    }));
}

/**
 * « Nouvelle conversation » : on choisit d'abord entre un groupe nommé et une
 * discussion, puis les membres.
 */
export function NewConversation({
  members,
  fronters,
  onDiscussion,
  onGroup,
}: {
  members: Member[];
  fronters: string[];
  onDiscussion: (draft: Draft) => void;
  onGroup: (group: ChatGroup) => void;
}) {
  const [open, setOpen] = useState(false);
  const [step, setStep] = useState<Step>("choose");
  const self = useSelfSystem(members);
  const friends = useFriendSystems();
  const [from, setFrom] = useState<SystemPick | null>(null);
  const [to, setTo] = useState<SystemPick | null>(null);
  const close = () => setOpen(false);
  // Vers un ami, on peut signer au nom du système entier ; entre nous, il faut
  // au moins un auteur.
  const toFriend = to !== null && to.system !== SELF;
  const valid = to !== null && from !== null && (toFriend || from.ids.length > 0);

  return (
    <>
      <Button
        variant="ghost"
        onPress={() => {
          setStep("choose");
          setFrom({ system: SELF, ids: fronters });
          setTo(null);
          setOpen(true);
        }}
      >
        <Icon name="plus" />
        {m.messages_new_conversation()}
      </Button>
      <ModalOverlay className="overlay" isDismissable isOpen={open} onOpenChange={setOpen}>
        <Modal className={step === "choose" ? "modal" : "modal modal--wide"}>
          <Dialog className="dialog">
            {step === "choose" ? (
              <>
                <Heading slot="title">{m.messages_new_conversation()}</Heading>
                <div className="new-conv">
                  <AriaButton className="new-conv__choice" onPress={() => setStep("discussion")}>
                    <Icon name="send" size={22} />
                    <span>
                      <strong>{m.messages_new_discussion()}</strong>
                      <span className="muted">{m.messages_new_discussion_lead()}</span>
                    </span>
                  </AriaButton>
                  <AriaButton className="new-conv__choice" onPress={() => setStep("group")}>
                    <Icon name="users" size={22} />
                    <span>
                      <strong>{m.group_new()}</strong>
                      <span className="muted">{m.group_new_lead()}</span>
                    </span>
                  </AriaButton>
                </div>
                <div className="dialog__actions">
                  <Button variant="ghost" slot="close">
                    {m.cancel()}
                  </Button>
                </div>
              </>
            ) : step === "discussion" ? (
              <>
                <div>
                  <Heading slot="title">{m.messages_new_discussion()}</Heading>
                  <p className="muted">{m.messages_new_discussion_pick()}</p>
                </div>
                <SystemsPicker
                  label={m.messages_pick_from()}
                  systems={[self]}
                  value={from}
                  onChange={setFrom}
                />
                <SystemsPicker
                  label={m.messages_pick_recipients()}
                  systems={[self, ...friends]}
                  value={to}
                  onChange={setTo}
                />
                {from && from.ids.length === 0 && !toFriend && to && (
                  <p className="field__hint">{m.messages_pick_from_hint()}</p>
                )}
                <div className="dialog__actions">
                  <Button variant="ghost" onPress={() => setStep("choose")}>
                    {m.messages_previous()}
                  </Button>
                  <span style={{ flex: 1 }} />
                  <Button
                    isDisabled={!valid}
                    onPress={() => {
                      if (!from || !to) return;
                      onDiscussion({
                        friendId: toFriend ? to.system : null,
                        from: from.ids,
                        to: to.ids,
                      });
                      close();
                    }}
                  >
                    {m.messages_start()}
                  </Button>
                </div>
              </>
            ) : (
              <GroupForm
                group={null}
                members={members}
                onCancel={() => setStep("choose")}
                onDone={(group) => {
                  onGroup(group);
                  close();
                }}
              />
            )}
          </Dialog>
        </Modal>
      </ModalOverlay>
    </>
  );
}
