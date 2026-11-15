import { useMutation, useQuery } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import {
  TextField as AriaTextField,
  Dialog,
  DialogTrigger,
  Heading,
  Input,
  Label,
  Modal,
  ModalOverlay,
} from "react-aria-components";
import { keys, queryClient } from "../../app/query";
import { errorMessage } from "../../i18n/errors";
import { api } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { IconButton } from "../../ui/IconButton";
import { matches, Search } from "../../ui/Search";
import { useMembers } from "./useMembers";

const MAX_ROLE = 40;

/** Le catalogue change : les membres aussi (renommage, suppression). */
function applied(next: string[]) {
  queryClient.setQueryData(keys.roles, next);
  void queryClient.invalidateQueries({ queryKey: keys.members });
}

/** Catalogue de rôles du système : ajouter, renommer, supprimer. */
export function RolesDialog() {
  return (
    <ModalOverlay className="overlay" isDismissable>
      <Modal className="modal">
        <Dialog className="dialog">
          {({ close }) => (
            <>
              <Heading slot="title">{m.roles_title()}</Heading>
              <p className="muted">{m.roles_lead()}</p>
              <RolesEditor />
              <div className="dialog__actions">
                <Button variant="primary" onPress={close}>
                  {m.close()}
                </Button>
              </div>
            </>
          )}
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}

function RolesEditor() {
  const roles = useQuery({ queryKey: keys.roles, queryFn: api.roles.list });
  const [draft, setDraft] = useState("");
  const add = useMutation({
    mutationFn: api.roles.add,
    onSuccess: (next) => {
      applied(next);
      setDraft("");
    },
  });
  const submit = () => {
    if (draft.trim()) add.mutate(draft.trim());
  };
  const list = roles.data ?? [];
  const members = useMembers().list;
  const [q, setQ] = useState("");
  const usage = useMemo(() => {
    const n = new Map<string, number>();
    for (const x of members) for (const r of x.roles) n.set(r, (n.get(r) ?? 0) + 1);
    return n;
  }, [members]);
  const shown = list.filter((r) => matches(q, { name: r }));

  return (
    <div className="field">
      {roles.isSuccess && list.length === 0 && <p className="muted">{m.roles_empty()}</p>}
      {list.length > 10 && <Search label={m.search_roles()} value={q} onChange={setQ} />}
      {list.length > 0 && shown.length === 0 && <p className="muted">{m.roles_none_found()}</p>}
      {shown.length > 0 && (
        <ul className="role-list">
          {shown.map((r) => (
            <RoleRow key={r} role={r} count={usage.get(r) ?? 0} />
          ))}
        </ul>
      )}
      <div className="roles__add">
        <AriaTextField
          className="field roles__input"
          maxLength={MAX_ROLE}
          value={draft}
          onChange={setDraft}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              submit();
            }
          }}
        >
          <Label className="sr-only">{m.roles_new()}</Label>
          <Input className="input" placeholder={m.member_role_placeholder()} />
        </AriaTextField>
        <Button variant="ghost" isDisabled={!draft.trim() || add.isPending} onPress={submit}>
          {m.member_role_add()}
        </Button>
      </div>
      {add.isError && (
        <p className="alert" role="alert">
          {errorMessage(add.error)}
        </p>
      )}
    </div>
  );
}

function RoleRow({ role, count }: { role: string; count: number }) {
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState(role);
  const rename = useMutation({
    mutationFn: (to: string) => api.roles.rename(role, to),
    onSuccess: (next) => {
      applied(next);
      setEditing(false);
    },
  });
  const save = () => {
    const to = name.trim();
    if (!to || to === role) setEditing(false);
    else rename.mutate(to);
  };

  if (editing) {
    return (
      <li className="role-row role-row--edit">
        <AriaTextField
          className="field roles__input"
          maxLength={MAX_ROLE}
          value={name}
          onChange={setName}
          autoFocus
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              save();
            } else if (e.key === "Escape") {
              e.preventDefault();
              setEditing(false);
            }
          }}
        >
          <Label className="sr-only">{m.roles_rename_label({ role })}</Label>
          <Input className="input" />
        </AriaTextField>
        <span className="row-actions">
          <IconButton icon="close" label={m.cancel()} onPress={() => setEditing(false)} />
          <IconButton
            icon="check"
            tone="accent"
            label={m.roles_save()}
            isDisabled={!name.trim() || rename.isPending}
            onPress={save}
          />
        </span>
        {rename.isError && (
          <span className="alert role-row__error" role="alert">
            {errorMessage(rename.error)}
          </span>
        )}
      </li>
    );
  }
  return (
    <li className="role-row">
      <span className="role-row__dot" aria-hidden />
      <span className="role-row__name">
        {role}
        <small className="muted">{m.roles_count({ count })}</small>
      </span>
      <span className="row-actions">
        <IconButton
          icon="edit"
          label={m.action_edit()}
          onPress={() => {
            setName(role);
            setEditing(true);
          }}
        />
        <RemoveRole role={role} />
      </span>
    </li>
  );
}

function RemoveRole({ role }: { role: string }) {
  const remove = useMutation({ mutationFn: () => api.roles.remove(role), onSuccess: applied });
  return (
    <DialogTrigger>
      <IconButton icon="trash" tone="danger" label={m.action_delete()} />
      <ModalOverlay className="overlay" isDismissable>
        <Modal className="modal">
          <Dialog className="dialog" role="alertdialog">
            {({ close }) => (
              <>
                <Heading slot="title">{m.roles_remove_title({ role })}</Heading>
                <p className="muted">{m.roles_remove_body()}</p>
                {remove.isError && (
                  <p className="alert" role="alert">
                    {errorMessage(remove.error)}
                  </p>
                )}
                <div className="dialog__actions">
                  <Button variant="ghost" onPress={close}>
                    {m.cancel()}
                  </Button>
                  <Button
                    variant="danger"
                    autoFocus
                    isDisabled={remove.isPending}
                    onPress={() => remove.mutate(undefined, { onSuccess: close })}
                  >
                    {m.roles_remove()}
                  </Button>
                </div>
              </>
            )}
          </Dialog>
        </Modal>
      </ModalOverlay>
    </DialogTrigger>
  );
}
