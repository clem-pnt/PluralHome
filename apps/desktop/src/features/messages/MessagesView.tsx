import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { useMembers } from "../members/useMembers";
import { Composer } from "./Composer";
import { MessageCard } from "./MessageCard";
import { useMessageBoard } from "./useMessages";

export function MessagesView() {
  const members = useMembers();
  const board = useMessageBoard();
  const list = board.data?.pages.flat() ?? [];

  return (
    <section className="view">
      <header className="view__header">
        <div>
          <h1>{m.messages_title()}</h1>
          <p className="view__lead">{m.messages_lead()}</p>
        </div>
      </header>
      <Composer />
      {board.isSuccess && list.length === 0 && <p className="muted">{m.messages_empty()}</p>}
      <div className="messages">
        {list.map((msg) => (
          <MessageCard key={msg.id} message={msg} byId={members.byId} />
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
