import { useMutation } from "@tanstack/react-query";
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
import { errorMessage } from "../../i18n/errors";
import { api, type Folder } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { IconButton } from "../../ui/IconButton";
import { Select } from "../../ui/Select";
import { AutoSort } from "./AutoSortDialog";
import { buildTree, type FolderTree, membersUnder, subtree } from "./folderTree";
import { MemberPicker } from "./MemberPicker";
import { setFolders, useFolders } from "./useFolders";
import { useMembers } from "./useMembers";

const MAX_FOLDER = 60;
const ROOT = "__root__";

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
                <AutoSort />
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
  const tree = buildTree(folders.list);
  const [draft, setDraft] = useState("");
  const create = useMutation({
    mutationFn: (name: string) => api.folders.create(name),
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
        <ul className="facts folders">
          {tree.flat.map((n) => (
            <FolderRow key={n.folder.id} folder={n.folder} depth={n.depth} tree={tree} />
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

function FolderRow({ folder, depth, tree }: { folder: Folder; depth: number; tree: FolderTree }) {
  const [mode, setMode] = useState<"view" | "edit" | "sub">("view");
  const [name, setName] = useState(folder.name);
  const parent = tree.parentOf(folder.id);
  const [to, setTo] = useState(parent ?? ROOT);
  const save = useMutation({
    mutationFn: async () => {
      const n = name.trim();
      let next = n && n !== folder.name ? await api.folders.rename(folder.id, n) : null;
      const target = to === ROOT ? null : to;
      if (target !== parent) next = await api.folders.move(folder.id, target);
      return next;
    },
    onSuccess: (next) => {
      if (next) setFolders(next);
      setMode("view");
    },
  });
  const [sub, setSub] = useState("");
  const addSub = useMutation({
    mutationFn: () => api.folders.create(sub.trim(), folder.id),
    onSuccess: (next) => {
      setFolders(next);
      setSub("");
      setMode("view");
    },
  });
  // On ne peut pas ranger un dossier dans lui-même ni dans ses sous-dossiers.
  const own = new Set(subtree(tree, folder.id));
  const targets = [
    { id: ROOT, label: m.folders_root() },
    ...tree.flat
      .filter((n) => !own.has(n.folder.id))
      .map((n) => ({ id: n.folder.id, label: n.path })),
  ];
  const count = membersUnder(tree, folder.id).size;
  const style = { paddingInlineStart: `calc(${depth} * var(--space-5))` };
  const keys = (submit: () => void) => (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      submit();
    } else if (e.key === "Escape") {
      e.preventDefault();
      setMode("view");
    }
  };

  if (mode === "edit") {
    return (
      <li className="folder__edit" style={style}>
        <AriaTextField
          className="field"
          maxLength={MAX_FOLDER}
          value={name}
          onChange={setName}
          autoFocus
          onKeyDown={keys(() => name.trim() && save.mutate())}
        >
          <Label className="field__label">{m.folders_rename_label({ folder: folder.name })}</Label>
          <Input className="input" />
        </AriaTextField>
        <Select label={m.folders_parent()} options={targets} value={to} onChange={setTo} />
        <div className="roles__add">
          <Button variant="ghost" onPress={() => setMode("view")}>
            {m.cancel()}
          </Button>
          <Button isDisabled={!name.trim() || save.isPending} onPress={() => save.mutate()}>
            {m.roles_save()}
          </Button>
        </div>
        {save.isError && (
          <p className="alert" role="alert">
            {errorMessage(save.error)}
          </p>
        )}
      </li>
    );
  }
  return (
    <li className="folder__item" style={style}>
      <div className="folder__row">
        {depth > 0 && <span aria-hidden className="folder__branch" />}
        <span className="folder__name">
          {folder.name}
          <small className="muted">{m.folders_count({ count })}</small>
        </span>
        <span className="row-actions">
          <FolderMembers folder={folder} />
          <IconButton
            icon="folderPlus"
            label={m.folders_add_sub({ folder: folder.name })}
            pressed={mode === "sub"}
            onPress={() => setMode(mode === "sub" ? "view" : "sub")}
          />
          <IconButton
            icon="edit"
            label={m.action_edit()}
            onPress={() => {
              setName(folder.name);
              setTo(parent ?? ROOT);
              setMode("edit");
            }}
          />
          <RemoveFolder folder={folder} hasChildren={own.size > 1} />
        </span>
      </div>
      {mode === "sub" && (
        <div className="roles__add folder__sub">
          <AriaTextField
            className="field roles__input"
            maxLength={MAX_FOLDER}
            value={sub}
            onChange={setSub}
            autoFocus
            onKeyDown={keys(() => sub.trim() && addSub.mutate())}
          >
            <Label className="sr-only">{m.folders_add_sub({ folder: folder.name })}</Label>
            <Input className="input" placeholder={m.folders_sub_placeholder()} />
          </AriaTextField>
          <Button
            variant="ghost"
            isDisabled={!sub.trim() || addSub.isPending}
            onPress={() => addSub.mutate()}
          >
            {m.folders_add()}
          </Button>
        </div>
      )}
      {addSub.isError && (
        <p className="alert" role="alert">
          {errorMessage(addSub.error)}
        </p>
      )}
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
      <IconButton icon="users" label={m.folders_members()} />
      <ModalOverlay className="overlay" isDismissable>
        <Modal className="modal modal--wide">
          <Dialog className="dialog">
            {({ close }) => (
              <>
                <Heading slot="title">{m.folders_members_title({ folder: folder.name })}</Heading>
                <p className="muted">{m.folders_members_lead()}</p>
                <MemberPicker
                  label={m.folders_members_title({ folder: folder.name })}
                  members={members.list}
                  value={selected}
                  onChange={setSelected}
                />
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

function RemoveFolder({ folder, hasChildren }: { folder: Folder; hasChildren: boolean }) {
  const remove = useMutation({
    mutationFn: () => api.folders.remove(folder.id),
    onSuccess: setFolders,
  });
  return (
    <DialogTrigger>
      <IconButton icon="trash" tone="danger" label={m.action_delete()} />
      <ModalOverlay className="overlay" isDismissable>
        <Modal className="modal">
          <Dialog className="dialog" role="alertdialog">
            {({ close }) => (
              <>
                <Heading slot="title">{m.folders_remove_title({ folder: folder.name })}</Heading>
                <p className="muted">
                  {hasChildren ? m.folders_remove_body_sub() : m.folders_remove_body()}
                </p>
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
