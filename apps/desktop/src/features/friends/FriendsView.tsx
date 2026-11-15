import { useMutation, useQuery } from "@tanstack/react-query";
import { Tab, TabList, TabPanel, Tabs } from "react-aria-components";
import { keys } from "../../app/query";
import { api } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { AddFriendPanel } from "./AddFriend";
import { FriendCard } from "./FriendCard";
import { FriendError } from "./FriendError";
import { InvitePanel } from "./InvitePanel";
import { PrivacyPanel } from "./PrivacyPanel";
import { RequestsPanel, useRequests } from "./RequestsPanel";
import { refreshFriends, useFriends } from "./useFriends";

/** Amis : autres systèmes, ajoutés par lien ou QR code, avec qui échanger des messages. */
export function FriendsView() {
  const account = useQuery({ queryKey: keys.account, queryFn: api.account.get });
  const sync = useMutation({ mutationFn: api.friends.syncNow, onSuccess: refreshFriends });

  return (
    <section className="view">
      <header className="view__header">
        <div>
          <h1>{m.friends_title()}</h1>
          <p className="view__lead">{m.friends_lead()}</p>
        </div>
        {account.data && (
          <Button variant="ghost" isDisabled={sync.isPending} onPress={() => sync.mutate()}>
            {sync.isPending ? m.working() : m.friends_sync()}
          </Button>
        )}
      </header>
      <FriendError error={sync.error} />
      {account.isPending ? null : account.data ? (
        <FriendsTabs />
      ) : (
        <section className="panel">
          <p className="muted">{m.friends_need_account()}</p>
        </section>
      )}
    </section>
  );
}

function FriendsTabs() {
  const requests = useRequests();
  const incoming = requests.data?.incoming.length ?? 0;
  return (
    <Tabs className="friends" defaultSelectedKey="list">
      <TabList className="tabs" aria-label={m.friends_title()}>
        <Tab id="list" className="tab">
          {m.friends_tab_list()}
        </Tab>
        <Tab id="add" className="tab">
          {m.friends_tab_add()}
        </Tab>
        <Tab id="requests" className="tab">
          {incoming > 0
            ? m.friends_tab_requests_count({ count: incoming })
            : m.friends_tab_requests()}
        </Tab>
        <Tab id="privacy" className="tab">
          {m.friends_tab_privacy()}
        </Tab>
      </TabList>
      <TabPanel id="list" className="friends__panel">
        <FriendsList />
      </TabPanel>
      <TabPanel id="add" className="friends__panel">
        <AddFriendPanel />
        <InvitePanel />
      </TabPanel>
      <TabPanel id="requests" className="friends__panel">
        <RequestsPanel />
      </TabPanel>
      <TabPanel id="privacy" className="friends__panel">
        <PrivacyPanel />
      </TabPanel>
    </Tabs>
  );
}

function FriendsList() {
  const friends = useFriends();
  if (friends.isPending) return null;
  if (friends.error) return <FriendError error={friends.error} />;
  if (friends.list.length === 0)
    return (
      <section className="panel">
        <p className="muted">{m.friends_none()}</p>
      </section>
    );
  return (
    <ul className="friends-cards">
      {friends.list.map((f) => (
        <FriendCard key={f.id} friend={f} />
      ))}
    </ul>
  );
}
