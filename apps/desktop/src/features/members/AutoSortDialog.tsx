import { useMutation, useQuery } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import {
  Checkbox,
  CheckboxGroup,
  Dialog,
  DialogTrigger,
  Heading,
  Modal,
  ModalOverlay,
} from "react-aria-components";
import { keys, queryClient } from "../../app/query";
import { errorMessage } from "../../i18n/errors";
import { api, type Folder, type Member } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Avatar } from "../../ui/Avatar";
import { Button } from "../../ui/Button";
import { analyze, type Labels, type Proposal } from "./autoSort";
import { setFolders, useFolders } from "./useFolders";
import { useMembers } from "./useMembers";

const labels = (): Labels => ({
  parents: { age: m.sort_age_parent(), gender: m.sort_gender_parent() },
  age: {
    child: m.sort_age_child(),
    teen: m.sort_age_teen(),
    adult: m.sort_age_adult(),
    slider: m.sort_age_slider(),
  },
  gender: {
    female: m.sort_gender_female(),
    male: m.sort_gender_male(),
    nonbinary: m.sort_gender_nonbinary(),
    agender: m.sort_gender_agender(),
    fluid: m.sort_gender_fluid(),
  },
  roles: {
    host: m.role_host(),
    cohost: m.role_cohost(),
    protector: m.role_protector(),
    gatekeeper: m.role_gatekeeper(),
    little: m.role_little(),
    caretaker: m.role_caretaker(),
    trauma: m.role_trauma(),
    persecutor: m.role_persecutor(),
    introject: m.role_introject(),
    fictive: m.role_fictive(),
    fragment: m.role_fragment(),
  },
});

const keyOf = (p: Proposal) => `${p.kind}:${p.label}`;
const same = (a: string, b: string) => a.toLocaleLowerCase() === b.toLocaleLowerCase();

/** Bouton et fenêtre du tri automatique à partir des descriptions. */
export function AutoSort() {
  const [open, setOpen] = useState(false);
  return (
    <DialogTrigger isOpen={open} onOpenChange={setOpen}>
      <Button variant="ghost">{m.sort_open()}</Button>
      <ModalOverlay className="overlay" isDismissable>
        <Modal className="modal modal--wide">
          <Dialog className="dialog">{({ close }) => <AutoSortBody onDone={close} />}</Dialog>
        </Modal>
      </ModalOverlay>
    </DialogTrigger>
  );
}

function AutoSortBody({ onDone }: { onDone: () => void }) {
  const members = useMembers();
  const folders = useFolders();
  const catalog = useQuery({ queryKey: keys.roles, queryFn: api.roles.list });
  // Le catalogue sert à reprendre les libellés du système (« Protectrice » plutôt que « Protection »).
  if (members.isPending || folders.isPending || catalog.isPending) {
    return <p className="muted">{m.sort_loading()}</p>;
  }
  return <AutoSortForm catalog={catalog.data ?? []} onDone={onDone} />;
}

function AutoSortForm({ catalog, onDone }: { catalog: string[]; onDone: () => void }) {
  const members = useMembers();
  const folders = useFolders();
  // Calculé une fois à l'ouverture : la liste ne bouge pas pendant qu'on coche.
  const [proposals] = useState(() => analyze(members.list, folders.list, catalog, labels()));
  const [picked, setPicked] = useState<Record<string, string[]>>(() =>
    Object.fromEntries(proposals.map((p) => [keyOf(p), p.hits.map((h) => h.id)])),
  );
  const byId = useMemo(() => new Map(members.list.map((x) => [x.id, x])), [members.list]);
  const total =
    Object.values(picked).reduce((n, ids) => n + ids.length, 0) +
    proposals.filter((p) => p.move).length;

  const apply = useMutation({
    mutationFn: async () => {
      let current = folders.list;
      for (const p of proposals) {
        const ids = picked[keyOf(p)] ?? [];
        if (p.kind === "role" || (ids.length === 0 && !p.move)) continue;
        // Le dossier parent (« Âge », « Genre ») d'abord, créé au besoin.
        let parent: Folder | undefined;
        if (p.parent) {
          const name = p.parent;
          parent =
            current.find((f) => !f.parentId && same(f.name, name)) ??
            current.find((f) => same(f.name, name));
          if (!parent) {
            const before = new Set(current.map((f) => f.id));
            current = await api.folders.create(name);
            setFolders(current);
            parent = current.find((f) => !before.has(f.id));
          }
        }
        const parentId = parent?.id ?? null;
        let folder = p.folderId
          ? current.find((f) => f.id === p.folderId)
          : current.find((f) => f.parentId === parentId && same(f.name, p.label));
        if (folder && parentId && folder.parentId !== parentId && folder.id !== parentId) {
          current = await api.folders.move(folder.id, parentId);
          setFolders(current);
        }
        if (!folder) {
          const before = new Set(current.map((f) => f.id));
          current = await api.folders.create(p.label, parentId);
          setFolders(current);
          folder = current.find((f) => !before.has(f.id));
        }
        if (!folder || ids.length === 0) continue;
        const union = [...new Set([...folder.members, ...ids])];
        current = await api.folders.setMembers(folder.id, union);
        setFolders(current);
      }
      const roles = new Map<string, string[]>();
      for (const p of proposals) {
        if (p.kind !== "role") continue;
        for (const id of picked[keyOf(p)] ?? []) roles.set(id, [...(roles.get(id) ?? []), p.label]);
      }
      for (const [id, add] of roles) {
        const mb = byId.get(id);
        if (!mb) continue;
        const next = [...mb.roles];
        for (const r of add) if (!next.some((x) => same(x, r))) next.push(r);
        const saved = await api.members.setProfile(id, {
          description: mb.description,
          roles: next,
          avatar: { kind: "keep" },
        });
        queryClient.setQueryData<Member[]>(keys.members, (prev = []) =>
          prev.map((x) => (x.id === saved.id ? saved : x)),
        );
      }
      if (roles.size > 0) void queryClient.invalidateQueries({ queryKey: keys.roles });
    },
    onSuccess: onDone,
  });

  const section = (kind: Proposal["kind"], title: string) => {
    const list = proposals.filter((p) => p.kind === kind);
    if (list.length === 0) return null;
    return (
      <section className="field">
        <h3 className="sort__title">{title}</h3>
        {list.map((p) => (
          <div key={keyOf(p)} className="sort__group">
            <p className="sort__label">
              {p.label}
              {p.parent && <small className="muted">{m.sort_in({ folder: p.parent })}</small>}
              {kind !== "role" && !p.folderId && <small className="muted">{m.sort_new()}</small>}
              {p.move && p.parent && (
                <small className="muted">{m.sort_move({ folder: p.parent })}</small>
              )}
            </p>
            <CheckboxGroup
              className="picker"
              aria-label={p.label}
              value={picked[keyOf(p)] ?? []}
              onChange={(ids) => setPicked((prev) => ({ ...prev, [keyOf(p)]: ids }))}
            >
              {p.hits.map((h) => (
                <Checkbox key={h.id} value={h.id} className="pick">
                  <Avatar name={h.name} src={byId.get(h.id)?.avatar ?? null} size={32} />
                  <span className="pick__name">
                    {h.name}
                    <small className="muted">« {h.reason} »</small>
                  </span>
                </Checkbox>
              ))}
            </CheckboxGroup>
          </div>
        ))}
      </section>
    );
  };

  return (
    <>
      <Heading slot="title">{m.sort_title()}</Heading>
      <p className="muted">{m.sort_lead()}</p>
      {proposals.length === 0 ? (
        <p className="muted">{m.sort_empty()}</p>
      ) : (
        <>
          {section("age", m.sort_section_age())}
          {section("gender", m.sort_section_gender())}
          {section("role", m.sort_section_roles())}
        </>
      )}
      {apply.isError && (
        <p className="alert" role="alert">
          {errorMessage(apply.error)}
        </p>
      )}
      <div className="dialog__actions">
        <Button variant="ghost" onPress={onDone}>
          {proposals.length === 0 ? m.close() : m.cancel()}
        </Button>
        {proposals.length > 0 && (
          <Button isDisabled={total === 0 || apply.isPending} onPress={() => apply.mutate()}>
            {m.sort_apply({ count: total })}
          </Button>
        )}
      </div>
    </>
  );
}
