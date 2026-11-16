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
import type { ChatGroup, Friend, Member, Message, RemoteRef, SystemProfile } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Avatar } from "../../ui/Avatar";
import { AvatarStack } from "../../ui/AvatarStack";
import { Button } from "../../ui/Button";
import { Icon } from "../../ui/Icon";
import { IconButton } from "../../ui/IconButton";
import { matches, Search } from "../../ui/Search";
import { useFrontCurrent } from "../front/useFront";
import { useMembers } from "../members/useMembers";
import { GroupForm, type GroupSeed } from "./GroupForm";
import { MessageActions } from "./MessageCard";
import { type Draft, NewConversation } from "./NewConversation";
import { PeopleField } from "./PeopleField";
import {
  concerns,
  conversationOf,
  EVERYONE,
  friendKey,
  fronting,
  groupKey,
  names,
  ourSide,
  participantsKey,
  presentIds,
  readersOf,
} from "./people";
import { useGroups, useSystem } from "./useGroups";
import { MAX_BODY, useMarkReadBy, useMessageBoard, usePostMessage } from "./useMessages";
import {
  authorFace,
  authorKey,
  authorLabel,
  friendLabel,
  remoteFaces,
  remoteNames,
  systemLabel,
  useWho,
  type Who,
} from "./who";

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
  /** Échange avec un système ami ; `ids` est alors notre côté. */
  friendId?: string;
  /** Membres de l'ami dans l'échange ; vide : tout son système. */
  remote?: RemoteRef[];
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
  const who = useWho();
  const fronters = fronting(front.data);
  const present = presentIds(front.data);
  const [selected, setSelected] = useState<string | null>(null);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [query, setQuery] = useState("");
  /** Message à amener à l'écran (recherche, épinglé, citation). */
  const [focusId, setFocusId] = useState<string | null>(null);
  const [replying, setReplying] = useState<Message | null>(null);
  const [editing, setEditing] = useState<ChatGroup | null>(null);
  /** Discussion privée en train de devenir un groupe. */
  const [seed, setSeed] = useState<GroupSeed | null>(null);

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
        ...(group.friendId ? { friendId: group.friendId, remote: group.remoteMembers } : {}),
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
          conv = msg.friendId
            ? {
                key,
                ids: ourSide(msg),
                friendId: msg.friendId,
                remote: msg.outgoing ? msg.remoteRecipients : msg.remoteAuthors,
                messages: [],
                last: 0,
                unread: 0,
              }
            : { key, ids: key.split(","), messages: [], last: 0, unread: 0 };
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
    if (conv.friendId) {
      const friend = who.friends.get(conv.friendId);
      const list = remoteNames(conv.remote ?? [], friend);
      const sys = friendLabel(friend);
      return list ? m.messages_friend_people({ names: list, system: sys }) : sys;
    }
    return names(others(conv.ids), members.byId, 3) || m.messages_someone();
  };
  // Un groupe montre deux de ses membres, de préférence absents, comme Discord.
  const faceIds = (conv: Conversation) => {
    if (!conv.group) return others(conv.ids);
    const rest = conv.ids.filter((id) => !present.includes(id));
    return rest.length >= 2 ? rest : conv.ids;
  };
  // Groupe partagé : un visage de chaque système, comme une discussion à deux.
  const sharedFaces = (conv: Conversation): Person[] => {
    const friend = who.friends.get(conv.friendId ?? "");
    const ours = peopleOf(faceIds(conv), members.byId)[0] ?? {
      name: systemLabel(system.data),
      avatar: system.data?.avatar ?? null,
    };
    const theirs = remoteFaces(conv.remote ?? [], friend)[0] ?? {
      name: friendLabel(friend),
      avatar: friend?.avatar ?? null,
    };
    return [ours, theirs];
  };
  /** Les deux côtés d'un groupe partagé, pour son sous-titre. */
  const sharedSides = (group: ChatGroup) => {
    const friend = who.friends.get(group.friendId ?? "");
    const theirs = remoteNames(group.remoteMembers, friend);
    // Les autres systèmes du groupe : nos amis d'abord, puis ceux connus par un relais.
    const more = group.peers.map((p) => {
      const f = p.friendId ? who.friends.get(p.friendId) : undefined;
      const sys = p.friendId ? friendLabel(f) : (p.name ?? m.messages_friend_gone());
      const list = remoteNames(p.members, f);
      return list ? m.messages_friend_people({ names: list, system: sys }) : sys;
    });
    return [
      names(group.memberIds, members.byId, 3) || systemLabel(system.data),
      theirs
        ? m.messages_friend_people({ names: theirs, system: friendLabel(friend) })
        : friendLabel(friend),
      ...more,
    ].join(" · ");
  };
  const face = (conv: Conversation, size: number) => (
    <ConvFace
      conv={conv}
      people={
        conv.friendId && !conv.group
          ? remoteFaces(conv.remote ?? [], who.friends.get(conv.friendId))
          : conv.friendId
            ? sharedFaces(conv)
            : peopleOf(faceIds(conv), members.byId)
      }
      friend={conv.friendId ? who.friends.get(conv.friendId) : undefined}
      system={system.data}
      size={size}
    />
  );

  /** Modifier une conversation : son groupe, ou la discussion qui en deviendra un. */
  const edit = (conv: Conversation) => {
    if (conv.group) return setEditing(conv.group);
    if (conv.key === EVERYONE) return;
    setSeed({
      name: title(conv),
      memberIds: conv.ids,
      ...(conv.friendId ? { friendId: conv.friendId } : {}),
      remoteRefs: (conv.remote ?? []).map((r) => r.ref),
      messageIds: conv.messages.map((x) => x.id),
    });
  };
  const editable = (conv: Conversation) => conv.key !== EVERYONE;

  const startDiscussion = (next: Draft) => {
    // Une discussion avec les mêmes personnes existe déjà : on la rouvre.
    if (!next.friendId && next.to.length === 0) return select(EVERYONE);
    const keys = next.friendId
      ? [friendKey(next.friendId, next.from, next.to)]
      : [participantsKey([...next.from, ...next.to]), participantsKey(next.to)];
    const existing = conversations.find((c) => !c.group && keys.includes(c.key));
    if (existing) return select(existing.key);
    setDraft(next);
    select(DRAFT);
  };
  const draftConv = draft ? draftConversation(draft, who.friends.get(draft.friendId ?? "")) : null;

  const q = query.trim();
  const results = q
    ? conversations
        .flatMap((conv) =>
          conv.messages.filter((x) => matches(q, { name: x.body })).map((msg) => ({ conv, msg })),
        )
        .sort((a, b) => (a.msg.id < b.msg.id ? 1 : -1))
    : [];

  return (
    <section className="view view--messages" aria-label={m.messages_title()}>
      <div className="dm" data-open={selected !== null || undefined}>
        <nav className="dm__list" aria-label={m.messages_conversations()}>
          <div className="dm__list-head">
            <NewConversation
              members={members.list}
              fronters={fronters}
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
              who={who}
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
                      editable(conv)
                        ? (e) => {
                            e.preventDefault();
                            edit(conv);
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
                              ? group.friendId
                                ? sharedSides(group)
                                : m.group_member_count({ count: group.memberIds.length })
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
          {selected === DRAFT && draft && draftConv ? (
            <>
              <div className="dm__pane-head">
                <span className="dm__back">
                  <IconButton icon="back" label={m.messages_back()} onPress={() => select(null)} />
                </span>
                {face(draftConv, 28)}
                <h2 className="dm__pane-title">
                  {title(draftConv) || m.messages_new_discussion()}
                </h2>
              </div>
              <div className="dm__thread">
                <p className="muted dm__empty">{m.messages_conv_empty()}</p>
              </div>
              <ChatComposer
                conv={draftConv}
                title={title(draftConv)}
                defaultFrom={draft.from}
                members={members.list}
                who={who}
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
                      {current.group.friendId
                        ? sharedSides(current.group)
                        : names(current.group.memberIds, members.byId, 4)}
                    </span>
                  </AriaButton>
                ) : editable(current) ? (
                  <AriaButton
                    className="dm__pane-title dm__pane-title--button"
                    onPress={() => edit(current)}
                  >
                    <span className="dm__pane-name">{title(current)}</span>
                  </AriaButton>
                ) : (
                  <h2 className="dm__pane-title">{title(current)}</h2>
                )}
                {editable(current) && (
                  <span className="dm__pane-tools">
                    <IconButton
                      icon="edit"
                      label={
                        current.group
                          ? m.group_edit_title({ name: current.group.name })
                          : m.group_from_title()
                      }
                      onPress={() => edit(current)}
                    />
                  </span>
                )}
              </div>
              <PinnedBar
                key={`pins:${current.key}`}
                pinned={current.messages.filter((x) => x.pinned)}
                who={who}
                onOpen={(id) => setFocusId(id)}
              />
              <Thread
                conv={current}
                members={members.list}
                who={who}
                msgById={msgById}
                fronters={fronters}
                focusId={focusId}
                onFocus={setFocusId}
                onReply={setReplying}
              />
              <ChatComposer
                key={current.key}
                conv={current}
                title={title(current)}
                defaultFrom={current.friendId && !current.group ? current.ids : fronters}
                members={members.list}
                who={who}
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
        isOpen={editing !== null || seed !== null}
        onOpenChange={(open) => {
          if (open) return;
          setEditing(null);
          setSeed(null);
        }}
      >
        <Modal className="modal modal--wide">
          <Dialog className="dialog">
            {seed && (
              <GroupForm
                group={null}
                seed={seed}
                members={members.list}
                onCancel={() => setSeed(null)}
                onDone={(group) => {
                  setSeed(null);
                  select(groupKey(group.id));
                }}
              />
            )}
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
  friend,
  system,
  size,
}: {
  conv: Conversation;
  people: Person[];
  friend: Friend | undefined;
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
  // Avec tout le système d'un ami : son avatar de système.
  if (conv.friendId && !conv.group && people.length === 0) {
    return <Avatar name={friendLabel(friend)} src={friend?.avatar} size={size} />;
  }
  return <AvatarStack people={people} size={size} />;
}

function SearchResults({
  results,
  who,
  title,
  onOpen,
}: {
  results: { conv: Conversation; msg: Message }[];
  who: Who;
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
                <AuthorAvatar message={msg} who={who} size={32} />
                <span className="dm__conv-text">
                  <span className="dm__result-meta">
                    <span className="dm__conv-name">{authorLabel(msg, who, 2)}</span>
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
  who,
  onOpen,
}: {
  pinned: Message[];
  who: Who;
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
          {authorLabel(msg, who, 2)}
        </span>
        <span className="dm__pinned-body">{msg.body}</span>
      </span>
    </AriaButton>
  );
}

function Thread({
  conv,
  members,
  who,
  msgById,
  fronters,
  focusId,
  onFocus,
  onReply,
}: {
  conv: Conversation;
  members: Member[];
  who: Who;
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
          authorKey(prev) !== authorKey(msg) ||
          (msg.createdAt ?? 0) - (prev.createdAt ?? 0) > GROUP_MS;
        return (
          <ChatMessage
            key={msg.id}
            message={msg}
            parent={msg.replyTo ? (msgById.get(msg.replyTo) ?? null) : null}
            head={head}
            members={members}
            who={who}
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
  who,
  fronters,
  onReply,
  onOpenParent,
}: {
  message: Message;
  parent: Message | null;
  head: boolean;
  members: Member[];
  who: Who;
  fronters: string[];
  onReply: () => void;
  onOpenParent: (id: string) => void;
}) {
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
      {message.replyTo && <ReplyQuote parent={parent} who={who} onOpen={onOpenParent} />}
      <div className="chat-msg__gutter">
        {head && <AuthorAvatar message={message} who={who} size={36} />}
      </div>
      <div className="chat-msg__content">
        {head && (
          <header className="chat-msg__meta">
            <span className="chat-msg__author">{authorLabel(message, who)}</span>
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
              {m.messages_read_by({ names: names(message.readBy, who.byId, 4) })}
            </span>
          )}
        </div>
      </div>
      <div className="chat-msg__tools">
        <MessageActions
          message={message}
          byId={who.byId}
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
  who,
  onOpen,
}: {
  parent: Message | null;
  who: Who;
  onOpen: (id: string) => void;
}) {
  if (!parent) {
    return (
      <div className="chat-msg__reply">
        <span className="muted">{m.messages_reply_missing()}</span>
      </div>
    );
  }
  const author = authorFace(parent, who);
  const name = authorLabel(parent, who, 2);
  return (
    <AriaButton
      className="chat-msg__reply"
      aria-label={m.messages_reply_open({ name })}
      onPress={() => onOpen(parent.id)}
    >
      {author && <Avatar name={author.name} src={author.avatar} size={16} />}
      <span className="chat-msg__reply-author">{name}</span>
      <span className="chat-msg__reply-body muted">{parent.body}</span>
    </AriaButton>
  );
}

/** Avatar de la signature d'un message, de notre côté ou de celui d'un ami. */
function AuthorAvatar({ message, who, size }: { message: Message; who: Who; size: number }) {
  const face = authorFace(message, who);
  return face ? (
    <Avatar name={face.name} src={face.avatar} size={size} />
  ) : (
    <AvatarStack people={[]} size={size} />
  );
}

/** Conversation pas encore commencée, pour l'en-tête et la saisie. */
function draftConversation(draft: Draft, friend: Friend | undefined): Conversation {
  const base = { key: DRAFT, messages: [], last: 0, unread: 0 };
  if (!draft.friendId) return { ...base, ids: [...new Set([...draft.from, ...draft.to])] };
  return {
    ...base,
    ids: draft.from,
    friendId: draft.friendId,
    remote: draft.to.map((ref) => {
      const card = friend?.members.find((x) => x.id === ref);
      return { ref, name: card?.name ?? "", color: card?.color ?? "" };
    }),
  };
}

/** Saisie en bas du fil. Entrée envoie, Maj+Entrée va à la ligne. */
function ChatComposer({
  conv,
  title,
  defaultFrom,
  members,
  who,
  replyTo = null,
  onCancelReply,
  onSent,
}: {
  conv: Conversation;
  title: string;
  /** Qui signe par défaut : les personnes au front, ou notre côté de l'échange. */
  defaultFrom: string[];
  members: Member[];
  who: Who;
  replyTo?: Message | null;
  onCancelReply?: () => void;
  onSent: (message: Message) => void;
}) {
  const post = usePostMessage();
  const [authors, setAuthors] = useState<string[] | null>(null);
  const [body, setBody] = useState("");
  const input = useRef<HTMLTextAreaElement>(null);
  const from = authors ?? defaultFrom;
  const participants = conv.key === EVERYONE ? [] : conv.ids;
  const groupId = conv.group?.id ?? null;
  const friend = conv.friendId ? who.friends.get(conv.friendId) : undefined;

  // Répondre ramène le curseur dans la saisie.
  useEffect(() => {
    if (replyTo) input.current?.focus();
  }, [replyTo]);

  // L'ami a été retiré, ou n'accepte plus nos messages : on ne peut que relire.
  if (conv.friendId && !friend?.acceptsMessages) {
    return (
      <p className="muted dm__composer dm__composer--closed" role="status">
        {m.error_friend_closed()}
      </p>
    );
  }

  const send = () => {
    if (!body.trim() || post.isPending) return;
    const common = { body, groupId, replyTo: replyTo?.id ?? null };
    const payload = conv.friendId
      ? {
          ...common,
          // Personne de choisi : c'est tout le système qui signe.
          authorIds: from,
          bySystem: from.length === 0,
          recipientIds: [],
          friendId: conv.friendId,
          remoteRecipientRefs: groupId ? [] : (conv.remote ?? []).map((r) => r.ref),
        }
      : {
          ...common,
          authorIds: from,
          // Les destinataires : les autres participants (tous, si on s'écrit à soi-même).
          recipientIds: (() => {
            const others = participants.filter((id) => !from.includes(id));
            return participants.length === 0 ? [] : others.length > 0 ? others : participants;
          })(),
        };
    post.mutate(payload, {
      onSuccess: (msg) => {
        setBody("");
        onSent(msg);
      },
    });
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
            {m.messages_replying_to({ name: authorLabel(replyTo, who, 2) })}
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
          emptyLabel={conv.friendId ? m.messages_everyone() : m.messages_pick_none_author()}
          members={members}
          byId={who.byId}
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
              conv.group
                ? m.messages_placeholder_group({ name: conv.group.name })
                : conv.friendId
                  ? m.messages_placeholder_to({ names: title })
                  : participants.length === 0
                    ? m.messages_placeholder_everyone()
                    : m.messages_placeholder_to({ names: names(participants, who.byId) })
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
