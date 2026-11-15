import { useEffect, useRef, useState } from "react";
import {
  Button as AriaButton,
  Dialog,
  Form,
  Modal,
  ModalOverlay,
  TextArea,
  TextField,
} from "react-aria-components";
import { errorMessage } from "../../i18n/errors";
import { formatWhen } from "../../i18n/format";
import type { ChatGroup, Member, Message, SystemProfile } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Avatar } from "../../ui/Avatar";
import { AvatarStack } from "../../ui/AvatarStack";
import { Button } from "../../ui/Button";
import { Icon } from "../../ui/Icon";
import { IconButton } from "../../ui/IconButton";
import { matches, Search } from "../../ui/Search";
import { useFrontCurrent } from "../front/useFront";
import { useMembers } from "../members/useMembers";
import { GroupForm } from "./GroupForm";
import { MessageActions } from "./MessageCard";
import { NewConversation } from "./NewConversation";
import { PeopleField } from "./PeopleField";
import {
  concerns,
  conversationOf,
  EVERYONE,
  fronting,
  groupKey,
  names,
  participantsKey,
  presentIds,
  readersOf,
} from "./people";
import { useGroups, useSystem } from "./useGroups";
import { MAX_BODY, useMarkReadBy, useMessageBoard, usePostMessage } from "./useMessages";

/** Nouvelle discussion, tant qu'aucun message n'y a été envoyé. */
const DRAFT = "draft";
/** Deux messages d'un même auteur à moins de 7 minutes forment un seul bloc. */
const GROUP_MS = 7 * 60_000;
/** Au-delà, la recherche n'affiche que les résultats les plus récents. */
const MAX_RESULTS = 80;

interface Conversation {
  key: string;
  /** Participants (auteurs et destinataires, ou membres du groupe) ; vide pour tout le système. */
  ids: string[];
  group?: ChatGroup;
  /** Du plus ancien au plus récent. */
  messages: Message[];
  last: number;
  unread: number;
}

type Person = { name: string; avatar: string | null };

const peopleOf = (ids: string[], byId: Map<string, Member>): Person[] =>
  ids.flatMap((id) => {
    const x = byId.get(id);
    return x ? [{ name: x.name, avatar: x.avatar ?? null }] : [];
  });

/**
 * Messagerie à la manière des messages privés : les conversations à gauche, le
 * fil à droite, la saisie en bas. N'apparaît que ce qui concerne quelqu'un de
 * présent (front, co-front, co-conscience), ce qui est pour tout le système, et
 * les groupes nommés.
 */
export function MessagesView() {
  const members = useMembers();
  const front = useFrontCurrent();
  const board = useMessageBoard(false);
  const groups = useGroups();
  const system = useSystem();
  const fronters = fronting(front.data);
  const present = presentIds(front.data);
  const [selected, setSelected] = useState<string | null>(null);
  const [draftTo, setDraftTo] = useState<string[]>([]);
  const [query, setQuery] = useState("");
  /** Message à amener à l'écran (recherche, épinglé, citation). */
  const [focusId, setFocusId] = useState<string | null>(null);
  const [replying, setReplying] = useState<Message | null>(null);
  const [editing, setEditing] = useState<ChatGroup | null>(null);

  // L'historique d'un système reste petit : on le charge en entier pour regrouper.
  useEffect(() => {
    if (board.hasNextPage && !board.isFetchingNextPage) void board.fetchNextPage();
  }, [board.hasNextPage, board.isFetchingNextPage, board.fetchNextPage]);

  const all = board.data?.pages.flat() ?? [];
  const msgById = new Map(all.map((x) => [x.id, x]));

  const conversations = (() => {
    const map = new Map<string, Conversation>();
    map.set(EVERYONE, { key: EVERYONE, ids: [], messages: [], last: 0, unread: 0 });
    for (const group of groups.data ?? []) {
      const key = groupKey(group.id);
      map.set(key, {
        key,
        ids: group.memberIds,
        group,
        messages: [],
        last: group.createdAt ?? 0,
        unread: 0,
      });
    }
    for (const msg of all) {
      const key = conversationOf(msg);
      let conv = map.get(key);
      if (msg.groupId) {
        // Groupe supprimé entre deux chargements : ses messages partent avec lui.
        if (!conv) continue;
      } else {
        if (!concerns(msg, present)) continue;
        if (!conv) {
          conv = { key, ids: key.split(","), messages: [], last: 0, unread: 0 };
          map.set(key, conv);
        }
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

  const select = (key: string | null, focus: string | null = null) => {
    if (key !== selected) setReplying(null);
    setSelected(key);
    setFocusId(focus);
  };

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

  const others = (ids: string[]) => {
    // Comme en message privé : on nomme les autres, pas soi.
    const rest = ids.filter((id) => !present.includes(id));
    return rest.length > 0 ? rest : ids;
  };
  const title = (conv: Conversation) => {
    if (conv.key === EVERYONE) return system.data?.name || m.messages_everyone();
    if (conv.group) return conv.group.name;
    return names(others(conv.ids), members.byId, 3) || m.messages_someone();
  };
  // Un groupe montre deux de ses membres, de préférence absents, comme Discord.
  const faceIds = (conv: Conversation) => {
    if (!conv.group) return others(conv.ids);
    const rest = conv.ids.filter((id) => !present.includes(id));
    return rest.length >= 2 ? rest : conv.ids;
  };
  const face = (conv: Conversation, size: number) => (
    <ConvFace
      conv={conv}
      people={peopleOf(faceIds(conv), members.byId)}
      system={system.data}
      size={size}
    />
  );

  const startDiscussion = (ids: string[]) => {
    // Une discussion avec les mêmes personnes existe déjà : on la rouvre.
    const keys = [participantsKey(ids), participantsKey([...ids, ...fronters])];
    const existing = conversations.find((c) => !c.group && keys.includes(c.key));
    if (existing) return select(existing.key);
    setDraftTo(ids);
    select(DRAFT);
  };

  const q = query.trim();
  const results = q
    ? conversations
        .flatMap((conv) =>
          conv.messages.filter((x) => matches(q, { name: x.body })).map((msg) => ({ conv, msg })),
        )
        .sort((a, b) => (a.msg.id < b.msg.id ? 1 : -1))
    : [];

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
            <NewConversation
              members={members.list}
              onDiscussion={startDiscussion}
              onGroup={(group) => select(groupKey(group.id))}
            />
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
            <Search
              label={m.messages_search()}
              placeholder={m.messages_search_placeholder()}
              value={query}
              onChange={setQuery}
            />
          </div>
          {q ? (
            <SearchResults
              results={results}
              byId={members.byId}
              title={title}
              onOpen={(conv, msg) => select(conv.key, msg.id)}
            />
          ) : (
            <ul className="dm__convs">
              {conversations.map((conv) => {
                const lastMsg = conv.messages.at(-1);
                const group = conv.group;
                return (
                  <li
                    key={conv.key}
                    onContextMenu={
                      group
                        ? (e) => {
                            e.preventDefault();
                            setEditing(group);
                          }
                        : undefined
                    }
                  >
                    <AriaButton
                      className="dm__conv"
                      data-current={conv === current || undefined}
                      onPress={() => select(conv.key)}
                    >
                      {face(conv, 36)}
                      <span className="dm__conv-text">
                        <span className="dm__conv-name">{title(conv)}</span>
                        <span className="dm__conv-last muted">
                          {lastMsg
                            ? lastMsg.body
                            : group
                              ? m.group_member_count({ count: group.memberIds.length })
                              : m.messages_conv_empty()}
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
          )}
        </nav>

        <div className="dm__pane">
          {selected === DRAFT ? (
            <>
              <div className="dm__pane-head">
                <span className="dm__back">
                  <IconButton icon="back" label={m.messages_back()} onPress={() => select(null)} />
                </span>
                <AvatarStack people={peopleOf(draftTo, members.byId)} size={28} />
                <h2 className="dm__pane-title">
                  {names(draftTo, members.byId, 3) || m.messages_new_discussion()}
                </h2>
              </div>
              <div className="dm__thread">
                <p className="muted dm__empty">{m.messages_conv_empty()}</p>
              </div>
              <ChatComposer
                participants={draftTo}
                members={members.list}
                byId={members.byId}
                fronters={fronters}
                onSent={(msg) => select(conversationOf(msg))}
              />
            </>
          ) : current ? (
            <>
              <div className="dm__pane-head">
                <span className="dm__back">
                  <IconButton icon="back" label={m.messages_back()} onPress={() => select(null)} />
                </span>
                {face(current, 28)}
                {current.group ? (
                  <AriaButton
                    className="dm__pane-title dm__pane-title--button"
                    onPress={() => setEditing(current.group ?? null)}
                  >
                    <span className="dm__pane-name">{current.group.name}</span>
                    <span className="muted dm__pane-sub">
                      {names(current.group.memberIds, members.byId, 4)}
                    </span>
                  </AriaButton>
                ) : (
                  <h2 className="dm__pane-title">{title(current)}</h2>
                )}
                {current.group && (
                  <span className="dm__pane-tools">
                    <IconButton
                      icon="edit"
                      label={m.group_edit_title({ name: current.group.name })}
                      onPress={() => setEditing(current.group ?? null)}
                    />
                  </span>
                )}
              </div>
              <PinnedBar
                key={`pins:${current.key}`}
                pinned={current.messages.filter((x) => x.pinned)}
                byId={members.byId}
                onOpen={(id) => setFocusId(id)}
              />
              <Thread
                conv={current}
                members={members.list}
                byId={members.byId}
                msgById={msgById}
                fronters={fronters}
                focusId={focusId}
                onFocus={setFocusId}
                onReply={setReplying}
              />
              <ChatComposer
                key={current.key}
                participants={current.ids}
                groupId={current.group?.id ?? null}
                groupName={current.group?.name}
                members={members.list}
                byId={members.byId}
                fronters={fronters}
                replyTo={replying}
                onCancelReply={() => setReplying(null)}
                onSent={(msg) => {
                  setReplying(null);
                  select(conversationOf(msg));
                }}
              />
            </>
          ) : null}
        </div>
      </div>

      <ModalOverlay
        className="overlay"
        isDismissable
        isOpen={editing !== null}
        onOpenChange={(open) => !open && setEditing(null)}
      >
        <Modal className="modal modal--wide">
          <Dialog className="dialog">
            {editing && (
              <GroupForm
                group={editing}
                members={members.list}
                onCancel={() => setEditing(null)}
                onDone={() => setEditing(null)}
                onDeleted={() => {
                  if (selected === groupKey(editing.id)) select(EVERYONE);
                  setEditing(null);
                }}
              />
            )}
          </Dialog>
        </Modal>
      </ModalOverlay>
    </section>
  );
}

/** Visage d'une conversation : système, groupe, une personne ou deux empilées. */
function ConvFace({
  conv,
  people,
  system,
  size,
}: {
  conv: Conversation;
  people: Person[];
  system: SystemProfile | undefined;
  size: number;
}) {
  if (conv.key === EVERYONE) {
    if (system?.avatar) return <Avatar name={system.name ?? ""} src={system.avatar} size={size} />;
    return (
      <span
        className="dm__everyone"
        style={{ "--size": `${size}px` } as React.CSSProperties}
        aria-hidden
      >
        <Icon name="users" size={Math.round(size / 2)} />
      </span>
    );
  }
  if (conv.group?.avatar) {
    return <Avatar name={conv.group.name} src={conv.group.avatar} size={size} />;
  }
  return <AvatarStack people={people} size={size} />;
}

function SearchResults({
  results,
  byId,
  title,
  onOpen,
}: {
  results: { conv: Conversation; msg: Message }[];
  byId: Map<string, Member>;
  title: (conv: Conversation) => string;
  onOpen: (conv: Conversation, msg: Message) => void;
}) {
  if (results.length === 0) {
    return (
      <p className="muted dm__search-note" role="status">
        {m.messages_search_none()}
      </p>
    );
  }
  return (
    <>
      <p className="muted dm__search-note" role="status">
        {m.messages_search_count({ count: results.length })}
      </p>
      <ul className="dm__convs">
        {results.slice(0, MAX_RESULTS).map(({ conv, msg }) => {
          const created = msg.createdAt ?? 0;
          return (
            <li key={msg.id}>
              <AriaButton className="dm__conv dm__result" onPress={() => onOpen(conv, msg)}>
                <AvatarStack people={peopleOf(msg.authorIds.slice(0, 1), byId)} size={32} />
                <span className="dm__conv-text">
                  <span className="dm__result-meta">
                    <span className="dm__conv-name">
                      {names(msg.authorIds, byId, 2) || m.messages_someone()}
                    </span>
                    <span className="muted">
                      {title(conv)} · {formatWhen(created)}
                    </span>
                  </span>
                  <span className="dm__result-body">{msg.body}</span>
                </span>
              </AriaButton>
            </li>
          );
        })}
      </ul>
    </>
  );
}

/**
 * Bandeau des messages épinglés, en haut du fil comme sur WhatsApp : il montre
 * le plus récent ; chaque clic y amène puis passe au précédent.
 */
function PinnedBar({
  pinned,
  byId,
  onOpen,
}: {
  pinned: Message[];
  byId: Map<string, Member>;
  onOpen: (id: string) => void;
}) {
  const [step, setStep] = useState(0);
  const n = pinned.length;
  if (n === 0) return null;
  const index = n - 1 - (step % n);
  const msg = pinned[index] as Message;
  return (
    <AriaButton
      className="dm__pinned"
      onPress={() => {
        onOpen(msg.id);
        setStep(step + 1);
      }}
    >
      <Icon name="pin" size={16} />
      <span className="dm__pinned-text">
        <span className="dm__pinned-label">
          {n > 1 ? m.messages_pinned_index({ index: index + 1, count: n }) : m.messages_pinned()}
          {" · "}
          {names(msg.authorIds, byId, 2) || m.messages_someone()}
        </span>
        <span className="dm__pinned-body">{msg.body}</span>
      </span>
    </AriaButton>
  );
}

function Thread({
  conv,
  members,
  byId,
  msgById,
  fronters,
  focusId,
  onFocus,
  onReply,
}: {
  conv: Conversation;
  members: Member[];
  byId: Map<string, Member>;
  msgById: Map<string, Message>;
  fronters: string[];
  focusId: string | null;
  onFocus: (id: string | null) => void;
  onReply: (message: Message) => void;
}) {
  const end = useRef<HTMLDivElement>(null);
  const count = conv.messages.length;
  // biome-ignore lint/correctness/useExhaustiveDependencies: on suit le nombre de messages.
  useEffect(() => {
    if (!focusId) end.current?.scrollIntoView({ block: "end" });
  }, [count, conv.key]);

  // Amène le message visé à l'écran et le fait brièvement ressortir.
  useEffect(() => {
    if (!focusId) return;
    const el = document.getElementById(`msg-${focusId}`);
    if (!el) return;
    el.scrollIntoView({ block: "center", behavior: "smooth" });
    el.setAttribute("data-flash", "");
    const t = setTimeout(() => {
      el.removeAttribute("data-flash");
      onFocus(null);
    }, 1600);
    return () => {
      clearTimeout(t);
      el.removeAttribute("data-flash");
    };
  }, [focusId, onFocus]);

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
          msg.replyTo != null ||
          prev.authorIds.join() !== msg.authorIds.join() ||
          (msg.createdAt ?? 0) - (prev.createdAt ?? 0) > GROUP_MS;
        return (
          <ChatMessage
            key={msg.id}
            message={msg}
            parent={msg.replyTo ? (msgById.get(msg.replyTo) ?? null) : null}
            head={head}
            members={members}
            byId={byId}
            fronters={fronters}
            onReply={() => onReply(msg)}
            onOpenParent={onFocus}
          />
        );
      })}
      <div ref={end} />
    </div>
  );
}

function ChatMessage({
  message,
  parent,
  head,
  members,
  byId,
  fronters,
  onReply,
  onOpenParent,
}: {
  message: Message;
  parent: Message | null;
  head: boolean;
  members: Member[];
  byId: Map<string, Member>;
  fronters: string[];
  onReply: () => void;
  onOpenParent: (id: string) => void;
}) {
  const author = byId.get(message.authorIds[0] ?? "");
  const created = message.createdAt ?? 0;
  const unread = readersOf(message, fronters).length > 0;
  return (
    <article
      id={`msg-${message.id}`}
      className="chat-msg"
      data-head={head || undefined}
      data-pinned={message.pinned || undefined}
      data-unread={unread || undefined}
      onDoubleClick={(e) => {
        // Double-clic pour répondre, sauf sur les boutons du message.
        if ((e.target as HTMLElement).closest("button")) return;
        window.getSelection()?.removeAllRanges();
        onReply();
      }}
    >
      {message.replyTo && <ReplyQuote parent={parent} byId={byId} onOpen={onOpenParent} />}
      <div className="chat-msg__gutter">
        {head &&
          (author ? (
            <Avatar name={author.name} src={author.avatar} size={36} />
          ) : (
            <AvatarStack people={[]} size={36} />
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
        <MessageActions
          message={message}
          byId={byId}
          members={members}
          fronters={fronters}
          onReply={onReply}
        />
      </div>
    </article>
  );
}

/** Citation du message auquel on répond, au-dessus de la réponse, comme sur Discord. */
function ReplyQuote({
  parent,
  byId,
  onOpen,
}: {
  parent: Message | null;
  byId: Map<string, Member>;
  onOpen: (id: string) => void;
}) {
  if (!parent) {
    return (
      <div className="chat-msg__reply">
        <span className="muted">{m.messages_reply_missing()}</span>
      </div>
    );
  }
  const author = byId.get(parent.authorIds[0] ?? "");
  return (
    <AriaButton
      className="chat-msg__reply"
      aria-label={m.messages_reply_open({
        name: names(parent.authorIds, byId, 2) || m.messages_someone(),
      })}
      onPress={() => onOpen(parent.id)}
    >
      {author && <Avatar name={author.name} src={author.avatar} size={16} />}
      <span className="chat-msg__reply-author">
        {names(parent.authorIds, byId, 2) || m.messages_someone()}
      </span>
      <span className="chat-msg__reply-body muted">{parent.body}</span>
    </AriaButton>
  );
}

/** Saisie en bas du fil. Entrée envoie, Maj+Entrée va à la ligne. */
function ChatComposer({
  participants,
  groupId = null,
  groupName,
  members,
  byId,
  fronters,
  replyTo = null,
  onCancelReply,
  onSent,
}: {
  participants: string[];
  groupId?: string | null;
  groupName?: string | undefined;
  members: Member[];
  byId: Map<string, Member>;
  fronters: string[];
  replyTo?: Message | null;
  onCancelReply?: () => void;
  onSent: (message: Message) => void;
}) {
  const post = usePostMessage();
  const [authors, setAuthors] = useState<string[] | null>(null);
  const [body, setBody] = useState("");
  const input = useRef<HTMLTextAreaElement>(null);
  // Par défaut, ce sont les personnes au front qui écrivent.
  const from = authors ?? fronters;

  // Répondre ramène le curseur dans la saisie.
  useEffect(() => {
    if (replyTo) input.current?.focus();
  }, [replyTo]);

  const send = () => {
    if (!body.trim() || post.isPending) return;
    // Les destinataires : les autres participants (tous, si on s'écrit à soi-même).
    const others = participants.filter((id) => !from.includes(id));
    const recipientIds = participants.length === 0 ? [] : others.length > 0 ? others : participants;
    post.mutate(
      { authorIds: from, recipientIds, body, groupId, replyTo: replyTo?.id ?? null },
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
      {replyTo && (
        <div className="dm__replying">
          <Icon name="reply" size={16} />
          <span className="dm__replying-text">
            {m.messages_replying_to({
              name: names(replyTo.authorIds, byId, 2) || m.messages_someone(),
            })}
            <span className="muted"> {replyTo.body}</span>
          </span>
          <IconButton
            icon="close"
            label={m.messages_reply_cancel()}
            onPress={() => onCancelReply?.()}
          />
        </div>
      )}
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
            ref={input}
            className="input textarea dm__composer-input"
            rows={1}
            placeholder={
              groupName
                ? m.messages_placeholder_group({ name: groupName })
                : participants.length === 0
                  ? m.messages_placeholder_everyone()
                  : m.messages_placeholder_to({ names: names(participants, byId) })
            }
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                send();
              } else if (e.key === "Escape" && replyTo) {
                e.preventDefault();
                onCancelReply?.();
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
