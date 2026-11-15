import { useMutation, useQuery } from "@tanstack/react-query";
import { useState } from "react";
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

  return (
    <div className="field">
      {roles.isSuccess && list.length === 0 && <p className="muted">{m.roles_empty()}</p>}
      {list.length > 0 && (
        <ul className="facts">
          {list.map((r) => (
            <RoleRow key={r} role={r} />
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

function RoleRow({ role }: { role: string }) {
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
      <li className="roles__add">
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
        <Button variant="ghost" isDisabled={!name.trim() || rename.isPending} onPress={save}>
          {m.roles_save()}
        </Button>
        {rename.isError && (
          <span className="alert" role="alert">
            {errorMessage(rename.error)}
          </span>
        )}
      </li>
    );
  }
  return (
    <li className="roles__add">
      <span className="chip">{role}</span>
      <Button
        variant="ghost"
        onPress={() => {
          setName(role);
          setEditing(true);
        }}
      >
        {m.roles_rename()}
      </Button>
      <RemoveRole role={role} />
    </li>
  );
}

function RemoveRole({ role }: { role: string }) {
  const remove = useMutation({ mutationFn: () => api.roles.remove(role), onSuccess: applied });
  return (
    <DialogTrigger>
      <Button variant="ghost">{m.roles_remove()}</Button>
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
