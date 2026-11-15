import { useState } from "react";
import { Button as AriaButton, Dialog, Heading, Modal, ModalOverlay } from "react-aria-components";
import type { ChatGroup, Member } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { Icon } from "../../ui/Icon";
import { MemberPicker } from "../members/MemberPicker";
import { GroupForm } from "./GroupForm";

type Step = "choose" | "discussion" | "group";

/**
 * « Nouvelle conversation » : on choisit d'abord entre un groupe nommé et une
 * discussion, puis les membres.
 */
export function NewConversation({
  members,
  onDiscussion,
  onGroup,
}: {
  members: Member[];
  onDiscussion: (ids: string[]) => void;
  onGroup: (group: ChatGroup) => void;
}) {
  const [open, setOpen] = useState(false);
  const [step, setStep] = useState<Step>("choose");
  const [ids, setIds] = useState<string[]>([]);
  const close = () => setOpen(false);

  return (
    <>
      <Button
        variant="ghost"
        onPress={() => {
          setStep("choose");
          setIds([]);
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
                <MemberPicker
                  label={m.messages_pick_recipients()}
                  members={members}
                  value={ids}
                  onChange={setIds}
                  autoFocus
                />
                <div className="dialog__actions">
                  <Button variant="ghost" onPress={() => setStep("choose")}>
                    {m.messages_previous()}
                  </Button>
                  <span style={{ flex: 1 }} />
                  <Button
                    isDisabled={ids.length === 0}
                    onPress={() => {
                      onDiscussion(ids);
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
