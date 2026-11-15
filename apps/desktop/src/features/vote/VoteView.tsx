import { useState } from "react";
import { m } from "../../paraglide/messages.js";
import { useFrontCurrent } from "../front/useFront";
import { useMembers } from "../members/useMembers";
import { PeopleField } from "../messages/PeopleField";
import { fronting } from "../messages/people";
import { NewPollDialog } from "./NewPollDialog";
import { PollCard } from "./PollCard";
import { usePolls } from "./usePolls";

export function VoteView() {
  const members = useMembers();
  const front = useFrontCurrent();
  const polls = usePolls();
  const fronters = fronting(front.data);
  // Par défaut, ce sont les personnes au front qui votent, tant qu'on n'a pas choisi.
  const [picked, setPicked] = useState<string[] | null>(null);
  const voters = picked ?? fronters;
  const list = polls.data ?? [];

  return (
    <section className="view">
      <header className="view__header">
        <div>
          <h1>{m.vote_title()}</h1>
          <p className="view__lead">{m.vote_lead()}</p>
        </div>
        <NewPollDialog author={fronters[0] ?? null} />
      </header>
      <div className="vote__voters">
        <PeopleField
          label={m.vote_voters()}
          title={m.vote_voters_title()}
          lead={m.vote_voters_lead()}
          emptyLabel={m.vote_voters_none()}
          members={members.list}
          byId={members.byId}
          value={voters}
          onChange={setPicked}
        />
      </div>
      {polls.isSuccess && list.length === 0 && <p className="muted">{m.vote_empty()}</p>}
      <div className="polls">
        {list.map((p) => (
          <PollCard key={p.id} poll={p} byId={members.byId} voters={voters} />
        ))}
      </div>
    </section>
  );
}
