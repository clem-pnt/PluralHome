import { useState } from "react";
import { Radio, RadioGroup } from "react-aria-components";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { useFrontCurrent } from "../front/useFront";
import { useMembers } from "../members/useMembers";
import { Composer } from "./Composer";
import { MessageCard } from "./MessageCard";
import { fronting } from "./people";
import { useMessageBoard } from "./useMessages";

type Mode = "unread" | "all";

export function MessagesView() {
  const members = useMembers();
  const front = useFrontCurrent();
  const [mode, setMode] = useState<Mode>("unread");
  const board = useMessageBoard(mode === "unread");
  const list = board.data?.pages.flat() ?? [];
  const fronters = fronting(front.data);

  return (
    <section className="view">
      <header className="view__header">
        <div>
          <h1>{m.messages_title()}</h1>
          <p className="view__lead">{m.messages_lead()}</p>
        </div>
      </header>
      <Composer />
      <RadioGroup
        className="seg messages__mode"
        orientation="horizontal"
        aria-label={m.messages_view_label()}
        value={mode}
        onChange={(v) => setMode(v as Mode)}
      >
        <Radio value="unread" className="seg__item">
          {m.messages_view_unread()}
        </Radio>
        <Radio value="all" className="seg__item">
          {m.messages_view_all()}
        </Radio>
      </RadioGroup>
      {board.isSuccess && list.length === 0 && (
        <p className="muted">
          {mode === "unread" ? m.messages_empty_unread() : m.messages_empty()}
        </p>
      )}
      <div className="messages">
        {list.map((msg) => (
          <MessageCard
            key={msg.id}
            message={msg}
            byId={members.byId}
            members={members.list}
            fronters={fronters}
          />
        ))}
      </div>
      {board.hasNextPage && (
        <Button
          variant="ghost"
          isDisabled={board.isFetchingNextPage}
          onPress={() => void board.fetchNextPage()}
        >
          {m.messages_more()}
        </Button>
      )}
    </section>
  );
}
