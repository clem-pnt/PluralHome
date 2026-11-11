import { useState } from "react";
import { formatDay, startOfDay } from "../../i18n/format";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { Select } from "../../ui/Select";
import { useMembers } from "../members/useMembers";
import { EntryCard } from "./EntryCard";
import { EntryForm } from "./EntryForm";
import { useJournal } from "./useJournal";

const EVERYONE = "__all__";

export function JournalView() {
  const members = useMembers();
  const [filter, setFilter] = useState(EVERYONE);
  const journal = useJournal(filter === EVERYONE ? null : filter);
  const list = journal.data?.pages.flat() ?? [];

  return (
    <section className="view">
      <header className="view__header">
        <div>
          <h1>{m.journal_title()}</h1>
          <p className="view__lead">{m.journal_lead()}</p>
        </div>
      </header>
      <EntryForm />
      {members.list.length > 0 && (
        <div className="journal__filter">
          <Select
            label={m.journal_filter()}
            options={[
              { id: EVERYONE, label: m.journal_filter_all() },
              ...members.list.map((x) => ({ id: x.id, label: x.name })),
            ]}
            value={filter}
            onChange={setFilter}
          />
        </div>
      )}
      {journal.isSuccess && list.length === 0 && (
        <p className="muted">
          {filter === EVERYONE ? m.journal_empty() : m.journal_empty_member()}
        </p>
      )}
      <div className="journal">
        {list.map((entry, i) => {
          const day = startOfDay(entry.createdAt ?? 0);
          const prev = list[i - 1];
          const newDay = !prev || startOfDay(prev.createdAt ?? 0) !== day;
          return (
            <div key={entry.id} className="journal__item">
              {newDay && <h2 className="journal__day">{formatDay(entry.createdAt ?? 0)}</h2>}
              <EntryCard entry={entry} byId={members.byId} />
            </div>
          );
        })}
      </div>
      {journal.hasNextPage && (
        <Button
          variant="ghost"
          isDisabled={journal.isFetchingNextPage}
          onPress={() => void journal.fetchNextPage()}
        >
          {m.messages_more()}
        </Button>
      )}
    </section>
  );
}
