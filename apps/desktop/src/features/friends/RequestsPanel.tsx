import { useMutation, useQuery } from "@tanstack/react-query";
import { keys } from "../../app/query";
import { formatWhen } from "../../i18n/format";
import { api } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Avatar } from "../../ui/Avatar";
import { Button } from "../../ui/Button";
import { FriendError } from "./FriendError";
import { refreshFriends } from "./useFriends";

export function useRequests() {
  return useQuery({ queryKey: keys.friendsRequests, queryFn: api.friends.requests });
}

/** Demandes reçues (accepter, refuser, bloquer) et envoyées (annuler). */
export function RequestsPanel() {
  const requests = useRequests();
  const accept = useMutation({ mutationFn: api.friends.accept, onSuccess: refreshFriends });
  const drop = useMutation({ mutationFn: api.friends.dropRequest, onSuccess: refreshFriends });
  const block = useMutation({
    mutationFn: ({ id, name }: { id: string; name: string | null }) =>
      api.friends.blockRequest(id, name),
    onSuccess: refreshFriends,
  });
  const busy = accept.isPending || drop.isPending || block.isPending;
  const incoming = requests.data?.incoming ?? [];
  const outgoing = requests.data?.outgoing ?? [];

  return (
    <div className="panel">
      <h2 className="panel__title">{m.friends_requests_in()}</h2>
      {incoming.length === 0 ? (
        <p className="muted">{m.friends_requests_none()}</p>
      ) : (
        <>
          <p className="muted">{m.friends_requests_in_hint()}</p>
          <ul className="friends-list">
            {incoming.map((r) => (
              <li key={r.id} className="friends-row">
                <Avatar name={r.name || "?"} src={r.avatar} size={40} />
                <div className="friends-row__text">
                  <strong>{r.name || m.friends_unnamed()}</strong>
                  <span className="muted">
                    {m.friends_code({ code: r.code })}
                    {r.members !== null && ` · ${m.friends_members_count({ count: r.members })}`}
                    {r.createdAt !== null && ` · ${formatWhen(r.createdAt)}`}
                  </span>
                </div>
                <div className="row-actions">
                  <Button isDisabled={busy} onPress={() => accept.mutate(r.id)}>
                    {m.friends_accept()}
                  </Button>
                  <Button variant="ghost" isDisabled={busy} onPress={() => drop.mutate(r.id)}>
                    {m.friends_decline()}
                  </Button>
                  <Button
                    variant="danger"
                    isDisabled={busy}
                    onPress={() => block.mutate({ id: r.id, name: r.name })}
                  >
                    {m.friends_block()}
                  </Button>
                </div>
              </li>
            ))}
          </ul>
        </>
      )}

      {outgoing.length > 0 && (
        <>
          <h3 className="friends__sub">{m.friends_requests_out()}</h3>
          <ul className="friends-list">
            {outgoing.map((r) => (
              <li key={r.id} className="friends-row">
                <Avatar name={r.name || "?"} src={r.avatar} size={40} />
                <div className="friends-row__text">
                  <strong>{r.name || m.friends_unnamed()}</strong>
                  <span className="muted">
                    {r.code && `${m.friends_code({ code: r.code })} · `}
                    {m.friends_requests_waiting()}
                  </span>
                </div>
                <div className="row-actions">
                  <Button variant="ghost" isDisabled={busy} onPress={() => drop.mutate(r.id)}>
                    {m.friends_cancel_request()}
                  </Button>
                </div>
              </li>
            ))}
          </ul>
        </>
      )}
      <FriendError error={accept.error ?? drop.error ?? block.error} />
    </div>
  );
}
