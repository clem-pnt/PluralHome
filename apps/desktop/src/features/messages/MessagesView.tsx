import { useEffect, useRef, useState } from "react";
import { Button as AriaButton, Form, TextArea, TextField } from "react-aria-components";
import { errorMessage } from "../../i18n/errors";
import { formatWhen } from "../../i18n/format";
import type { Member, Message } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Avatar } from "../../ui/Avatar";
import { Button } from "../../ui/Button";
import { Icon } from "../../ui/Icon";
import { IconButton } from "../../ui/IconButton";
import { useFrontCurrent } from "../front/useFront";
import { useMembers } from "../members/useMembers";
import { MessageActions } from "./MessageCard";
import { PeopleField } from "./PeopleField";
import {
  concerns,
  conversationOf,
  EVERYONE,
  fronting,
  names,
  presentIds,
  readersOf,
} from "./people";
import { MAX_BODY, useMarkReadBy, useMessageBoard, usePostMessage } from "./useMessages";

/** Nouvelle conversation, tant qu'aucun message n'y a été envoyé. */
const DRAFT = "draft";
/** Deux messages d'un même auteur à moins de 7 minutes forment un seul bloc. */
const GROUP_MS = 7 * 60_000;

interface Conversation {
  key: string;
  /** Participants (auteurs et destinataires) ; vide pour tout le système. */
  ids: string[];
  /** Du plus ancien au plus récent. */
  messages: Message[];
  last: number;
  unread: number;
}

/**
 * Messagerie à la manière des messages privés : les conversations à gauche, le
 * fil à droite, la saisie en bas. N'apparaît que ce qui concerne quelqu'un de
 * présent (front, co-front, co-conscience), et ce qui est pour tout le système.
 */
export function MessagesView() {
  const members = useMembers();
  const front = useFrontCurrent();
  const board = useMessageBoard(false);
  const fronters = fronting(front.data);
  const present = presentIds(front.data);
  const [selected, setSelected] = useState<string | null>(null);
  const [draftTo, setDraftTo] = useState<string[]>([]);

  // L'historique d'un système reste petit : on le charge en entier pour regrouper.
  useEffect(() => {
    if (board.hasNextPage && !board.isFetchingNextPage) void board.fetchNextPage();
  }, [board.hasNextPage, board.isFetchingNextPage, board.fetchNextPage]);

  const conversations = (() => {
    const all = board.data?.pages.flat() ?? [];
    const map = new Map<string, Conversation>();
    map.set(EVERYONE, { key: EVERYONE, ids: [], messages: [], last: 0, unread: 0 });
    for (const msg of all) {
      if (!concerns(msg, present)) continue;
      const key = conversationOf(msg);
      let conv = map.get(key);
      if (!conv) {
        conv = { key, ids: key.split(","), messages: [], last: 0, unread: 0 };
        map.set(key, conv);
      }
      conv.messages.push(msg);
      conv.last = Math.max(conv.last, msg.createdAt ?? 0);
      if (readersOf(msg, fronters).length > 0) conv.unread += 1;
    }
    for (const conv of map.values()) {
      conv.messages.sort((a, b) => (a.id < b.id ? -1 : 1));
    }
    const [everyone, ...rest] = [...map.values()];
    return [everyone as Conversation, ...rest.sort((a, b) => b.last - a.last)];
  })();

  // Sur grand écran, la première conversation s'ouvre d'office.
  const current =
    selected === DRAFT ? null : (conversations.find((c) => c.key === selected) ?? conversations[0]);
  const markRead = useMarkReadBy();
  const unreadAll = conversations.flatMap((c) =>
    c.messages.flatMap((msg) => {
      const readers = readersOf(msg, fronters);
      return readers.length > 0 ? [{ id: msg.id, members: readers }] : [];
    }),
  );
  const markAll = async () => {
    for (const x of unreadAll) await markRead.mutateAsync(x);
  };

  const title = (conv: Conversation) => {
    if (conv.key === EVERYONE) return m.messages_everyone();
    // Comme en message privé : on nomme les autres, pas soi.
    const others = conv.ids.filter((id) => !present.includes(id));
    return names(others.length > 0 ? others : conv.ids, members.byId, 3) || m.messages_someone();
  };
  const faceOf = (conv: Conversation) => {
    const others = conv.ids.filter((id) => !present.includes(id));
    return members.byId.get((others.length > 0 ? others : conv.ids)[0] ?? "");
  };

  return (
    <section className="view view--messages">
      <header className="view__header">
        <div>
          <h1>{m.messages_title()}</h1>
          <p className="view__lead">{m.messages_lead()}</p>
        </div>
      </header>
      <div className="dm" data-open={selected !== null || undefined}>
        <nav className="dm__list" aria-label={m.messages_conversations()}>
          <div className="dm__list-head">
            <Button
              variant="ghost"
              onPress={() => {
                setDraftTo([]);
                setSelected(DRAFT);
              }}
            >
              <Icon name="plus" />
              {m.messages_new_conversation()}
            </Button>
            {unreadAll.length > 0 && (
              <Button
                variant="ghost"
                isDisabled={markRead.isPending}
                onPress={() => void markAll()}
              >
                <Icon name="checks" />
                {m.messages_mark_all_read()}
              </Button>
            )}
          </div>
          <ul className="dm__convs">
            {conversations.map((conv) => {
              const face = faceOf(conv);
              const lastMsg = conv.messages.at(-1);
              return (
                <li key={conv.key}>
                  <AriaButton
                    className="dm__conv"
                    data-current={conv === current || undefined}
                    onPress={() => setSelected(conv.key)}
                  >
                    {conv.key === EVERYONE ? (
                      <span className="dm__everyone" aria-hidden>
                        <Icon name="users" size={18} />
                      </span>
                    ) : face ? (
                      <Avatar name={face.name} src={face.avatar} size={36} />
                    ) : (
                      <span
                        className="avatar avatar--blurry"
                        style={{ "--size": "36px" } as React.CSSProperties}
                        aria-hidden
                      />
                    )}
                    <span className="dm__conv-text">
                      <span className="dm__conv-name">{title(conv)}</span>
                      <span className="dm__conv-last muted">
                        {lastMsg ? lastMsg.body : m.messages_conv_empty()}
                      </span>
                    </span>
                    {conv.unread > 0 && (
                      <span
                        className="dm__badge"
                        role="img"
                        aria-label={m.messages_unread_count({ count: conv.unread })}
                      >
                        {conv.unread}
                      </span>
                    )}
                  </AriaButton>
                </li>
              );
            })}
          </ul>
        </nav>

        <div className="dm__pane">
          {selected === DRAFT ? (
            <>
              <div className="dm__pane-head">
                <IconButton
                  icon="back"
                  label={m.messages_back()}
                  onPress={() => setSelected(null)}
                />
                <h2 className="dm__pane-title">{m.messages_new_conversation()}</h2>
              </div>
              <div className="dm__draft">
                <PeopleField
                  label={m.messages_to()}
                  title={m.messages_pick_recipients()}
                  lead={m.messages_pick_recipients_lead()}
                  emptyLabel={m.messages_everyone()}
                  members={members.list}
                  byId={members.byId}
                  value={draftTo}
                  onChange={setDraftTo}
                />
              </div>
              <div className="dm__thread" />
              <ChatComposer
                participants={draftTo}
                members={members.list}
                byId={members.byId}
                fronters={fronters}
                onSent={(msg) => setSelected(conversationOf(msg))}
              />
            </>
          ) : current ? (
            <>
              <div className="dm__pane-head">
                <IconButton
                  icon="back"
                  label={m.messages_back()}
                  onPress={() => setSelected(null)}
                />
                <h2 className="dm__pane-title">{title(current)}</h2>
              </div>
              <Thread
                conv={current}
                members={members.list}
                byId={members.byId}
                fronters={fronters}
              />
              <ChatComposer
                key={current.key}
                participants={current.ids}
                members={members.list}
                byId={members.byId}
                fronters={fronters}
                onSent={(msg) => setSelected(conversationOf(msg))}
              />
            </>
          ) : null}
        </div>
      </div>
    </section>
  );
}

function Thread({
  conv,
  members,
  byId,
  fronters,
}: {
  conv: Conversation;
  members: Member[];
  byId: Map<string, Member>;
  fronters: string[];
}) {
  const end = useRef<HTMLDivElement>(null);
  const count = conv.messages.length;
  // biome-ignore lint/correctness/useExhaustiveDependencies: on suit le nombre de messages.
  useEffect(() => {
    end.current?.scrollIntoView({ block: "end" });
  }, [count, conv.key]);

  if (count === 0) {
    return (
      <div className="dm__thread">
        <p className="muted dm__empty">{m.messages_conv_empty()}</p>
      </div>
    );
  }
  return (
    <div className="dm__thread">
      {conv.messages.map((msg, i) => {
        const prev = conv.messages[i - 1];
        const head =
          !prev ||
          prev.authorIds.join() !== msg.authorIds.join() ||
          (msg.createdAt ?? 0) - (prev.createdAt ?? 0) > GROUP_MS;
        return (
          <ChatMessage
            key={msg.id}
            message={msg}
            head={head}
            members={members}
            byId={byId}
            fronters={fronters}
          />
        );
      })}
      <div ref={end} />
    </div>
  );
}

function ChatMessage({
  message,
  head,
  members,
  byId,
  fronters,
}: {
  message: Message;
  head: boolean;
  members: Member[];
  byId: Map<string, Member>;
  fronters: string[];
}) {
  const author = byId.get(message.authorIds[0] ?? "");
  const created = message.createdAt ?? 0;
  const unread = readersOf(message, fronters).length > 0;
  return (
    <article
      className="chat-msg"
      data-head={head || undefined}
      data-pinned={message.pinned || undefined}
      data-unread={unread || undefined}
    >
      <div className="chat-msg__gutter">
        {head &&
          (author ? (
            <Avatar name={author.name} src={author.avatar} size={36} />
          ) : (
            <span
              className="avatar avatar--blurry"
              style={{ "--size": "36px" } as React.CSSProperties}
              aria-hidden
            />
          ))}
      </div>
      <div className="chat-msg__content">
        {head && (
          <header className="chat-msg__meta">
            <span className="chat-msg__author">
              {names(message.authorIds, byId) || m.messages_someone()}
            </span>
            <time className="muted" dateTime={new Date(created).toISOString()}>
              {formatWhen(created)}
            </time>
          </header>
        )}
        <p className="chat-msg__body" data-selectable>
          {message.body}
          {message.editedAt != null && (
            <span className="muted chat-msg__edited"> ({m.messages_edited()})</span>
          )}
        </p>
        <div className="chat-msg__foot">
          {message.pinned && <span className="tag">{m.messages_pinned()}</span>}
          {message.readBy.length > 0 && (
            <span className="muted">
              {m.messages_read_by({ names: names(message.readBy, byId, 4) })}
            </span>
          )}
        </div>
      </div>
      <div className="chat-msg__tools">
        <MessageActions message={message} byId={byId} members={members} fronters={fronters} />
      </div>
    </article>
  );
}

/** Saisie en bas du fil. Entrée envoie, Maj+Entrée va à la ligne. */
function ChatComposer({
  participants,
  members,
  byId,
  fronters,
  onSent,
}: {
  participants: string[];
  members: Member[];
  byId: Map<string, Member>;
  fronters: string[];
  onSent: (message: Message) => void;
}) {
  const post = usePostMessage();
  const [authors, setAuthors] = useState<string[] | null>(null);
  const [body, setBody] = useState("");
  // Par défaut, ce sont les personnes au front qui écrivent.
  const from = authors ?? fronters;

  const send = () => {
    if (!body.trim() || post.isPending) return;
    // Les destinataires : les autres participants (tous, si on s'écrit à soi-même).
    const others = participants.filter((id) => !from.includes(id));
    const recipientIds = participants.length === 0 ? [] : others.length > 0 ? others : participants;
    post.mutate(
      { authorIds: from, recipientIds, body },
      {
        onSuccess: (msg) => {
          setBody("");
          onSent(msg);
        },
      },
    );
  };

  return (
    <Form
      className="dm__composer"
      onSubmit={(e) => {
        e.preventDefault();
        send();
      }}
    >
      <div className="dm__composer-from">
        <PeopleField
          label={m.messages_from()}
          title={m.messages_pick_authors()}
          lead={m.messages_pick_authors_lead()}
          emptyLabel={m.messages_pick_none_author()}
          members={members}
          byId={byId}
          value={from}
          onChange={setAuthors}
        />
      </div>
      <div className="dm__composer-row">
        <TextField
          className="dm__composer-field"
          aria-label={m.messages_body()}
          value={body}
          onChange={setBody}
          maxLength={MAX_BODY}
        >
          <TextArea
            className="input textarea dm__composer-input"
            rows={1}
            placeholder={
              participants.length === 0
                ? m.messages_placeholder_everyone()
                : m.messages_placeholder_to({ names: names(participants, byId) })
            }
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                send();
              }
            }}
          />
        </TextField>
        <IconButton
          icon="send"
          tone="accent"
          label={m.messages_send()}
          type="submit"
          isDisabled={!body.trim() || post.isPending}
        />
      </div>
      {post.isError && (
        <p className="alert" role="alert">
          {errorMessage(post.error)}
        </p>
      )}
    </Form>
  );
}
