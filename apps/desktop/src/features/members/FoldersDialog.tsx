import { useMutation } from "@tanstack/react-query";
import { useState } from "react";
import {
  TextField as AriaTextField,
  Checkbox,
  CheckboxGroup,
  Dialog,
  DialogTrigger,
  Heading,
  Input,
  Label,
  Modal,
  ModalOverlay,
} from "react-aria-components";
import { errorMessage } from "../../i18n/errors";
import { api, type Folder } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Avatar } from "../../ui/Avatar";
import { Button } from "../../ui/Button";
import { setFolders, useFolders } from "./useFolders";
import { useMembers } from "./useMembers";

const MAX_FOLDER = 60;

/** Dossiers du système : créer, renommer, supprimer, choisir qui y est rangé. */
export function FoldersDialog() {
  return (
    <ModalOverlay className="overlay" isDismissable>
      <Modal className="modal modal--wide">
        <Dialog className="dialog">
          {({ close }) => (
            <>
              <Heading slot="title">{m.folders_title()}</Heading>
              <p className="muted">{m.folders_lead()}</p>
              <FoldersEditor />
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

function FoldersEditor() {
  const folders = useFolders();
  const [draft, setDraft] = useState("");
  const create = useMutation({
    mutationFn: api.folders.create,
    onSuccess: (next) => {
      setFolders(next);
      setDraft("");
    },
  });
  const submit = () => {
    if (draft.trim()) create.mutate(draft.trim());
  };

  return (
    <div className="field">
      {folders.isSuccess && folders.list.length === 0 && (
        <p className="muted">{m.folders_empty()}</p>
      )}
      {folders.list.length > 0 && (
        <ul className="facts">
          {folders.list.map((f) => (
            <FolderRow key={f.id} folder={f} />
          ))}
        </ul>
      )}
      <div className="roles__add">
        <AriaTextField
          className="field roles__input"
          maxLength={MAX_FOLDER}
          value={draft}
          onChange={setDraft}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              submit();
            }
          }}
        >
          <Label className="sr-only">{m.folders_new()}</Label>
          <Input className="input" placeholder={m.folders_placeholder()} />
        </AriaTextField>
        <Button variant="ghost" isDisabled={!draft.trim() || create.isPending} onPress={submit}>
          {m.folders_add()}
        </Button>
      </div>
      {create.isError && (
        <p className="alert" role="alert">
          {errorMessage(create.error)}
        </p>
      )}
    </div>
  );
}

function FolderRow({ folder }: { folder: Folder }) {
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState(folder.name);
  const rename = useMutation({
    mutationFn: (to: string) => api.folders.rename(folder.id, to),
    onSuccess: (next) => {
      setFolders(next);
      setEditing(false);
    },
  });
  const save = () => {
    const to = name.trim();
    if (!to || to === folder.name) setEditing(false);
    else rename.mutate(to);
  };

  if (editing) {
    return (
      <li className="roles__add">
        <AriaTextField
          className="field roles__input"
          maxLength={MAX_FOLDER}
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
          <Label className="sr-only">{m.folders_rename_label({ folder: folder.name })}</Label>
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
    <li className="roles__add folder__row">
      <span className="folder__name">
        {folder.name}
        <small className="muted">{m.folders_count({ count: folder.members.length })}</small>
      </span>
      <FolderMembers folder={folder} />
      <Button
        variant="ghost"
        onPress={() => {
          setName(folder.name);
          setEditing(true);
        }}
      >
        {m.roles_rename()}
      </Button>
      <RemoveFolder folder={folder} />
    </li>
  );
}

/** Choisir les membres rangés dans un dossier. */
function FolderMembers({ folder }: { folder: Folder }) {
  const members = useMembers();
  const [selected, setSelected] = useState<string[]>(folder.members);
  const save = useMutation({
    mutationFn: () => api.folders.setMembers(folder.id, selected),
    onSuccess: setFolders,
  });
  return (
    <DialogTrigger onOpenChange={(open) => open && setSelected(folder.members)}>
      <Button variant="ghost">{m.folders_members()}</Button>
      <ModalOverlay className="overlay" isDismissable>
        <Modal className="modal modal--wide">
          <Dialog className="dialog">
            {({ close }) => (
              <>
                <Heading slot="title">{m.folders_members_title({ folder: folder.name })}</Heading>
                <p className="muted">{m.folders_members_lead()}</p>
                <CheckboxGroup
                  className="picker"
                  aria-label={m.folders_members_title({ folder: folder.name })}
                  value={selected}
                  onChange={setSelected}
                >
                  {members.list.map((x) => (
                    <Checkbox key={x.id} value={x.id} className="pick">
                      <Avatar name={x.name} src={x.avatar} size={32} />
                      <span className="pick__name">{x.name}</span>
                    </Checkbox>
                  ))}
                </CheckboxGroup>
                {save.isError && (
                  <p className="alert" role="alert">
                    {errorMessage(save.error)}
                  </p>
                )}
                <div className="dialog__actions">
                  <Button variant="ghost" onPress={close}>
                    {m.cancel()}
                  </Button>
                  <Button
                    isDisabled={save.isPending}
                    onPress={() => save.mutate(undefined, { onSuccess: close })}
                  >
                    {m.roles_save()}
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

function RemoveFolder({ folder }: { folder: Folder }) {
  const remove = useMutation({
    mutationFn: () => api.folders.remove(folder.id),
    onSuccess: setFolders,
  });
  return (
    <DialogTrigger>
      <Button variant="ghost">{m.roles_remove()}</Button>
      <ModalOverlay className="overlay" isDismissable>
        <Modal className="modal">
          <Dialog className="dialog" role="alertdialog">
            {({ close }) => (
              <>
                <Heading slot="title">{m.folders_remove_title({ folder: folder.name })}</Heading>
                <p className="muted">{m.folders_remove_body()}</p>
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
