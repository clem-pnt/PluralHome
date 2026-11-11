import { commands, type IpcError_Serialize } from "./bindings";

export type {
  Account_Serialize as Account,
  AvatarEdit,
  CatchUp_Serialize as CatchUp,
  Contact_Serialize as Contact,
  Entry_Serialize as Entry,
  FrontChange,
  FrontSession_Serialize as FrontSession,
  Member_Serialize as Member,
  Message_Serialize as Message,
  NewEntry_Deserialize as NewEntry,
  NewMember_Serialize as NewMember,
  PkSystem_Serialize as PkSystem,
  SafetyPlan_Serialize as SafetyPlan,
  Status_Serialize as PkStatus,
  VaultStatus,
} from "./bindings";

/** Erreur métier venant du cœur Rust, avec un code stable à traduire. */
export class AppError extends Error {
  readonly code: string;
  readonly field: string | null;
  constructor({ code, field }: IpcError_Serialize) {
    super(code);
    this.name = "AppError";
    this.code = code;
    this.field = field;
  }
}

type Result<T> = { status: "ok"; data: T } | { status: "error"; error: IpcError_Serialize };

async function unwrap<T>(p: Promise<Result<T>>): Promise<T> {
  const r = await p;
  if (r.status === "error") throw new AppError(r.error);
  return r.data;
}

/** Façade typée : chaque appel rejette avec une `AppError` en cas d'échec. */
export const api = {
  vault: {
    status: () => unwrap(commands.vaultStatus()),
    create: (passphrase: string) => unwrap(commands.vaultCreate(passphrase)),
    unlock: (passphrase: string) => unwrap(commands.vaultUnlock(passphrase)),
    lock: () => unwrap(commands.vaultLock()),
  },
  members: {
    list: () => unwrap(commands.membersList()),
    create: (input: Parameters<typeof commands.membersCreate>[0]) =>
      unwrap(commands.membersCreate(input)),
    update: (id: string, input: Parameters<typeof commands.membersUpdate>[1]) =>
      unwrap(commands.membersUpdate(id, input)),
    setProfile: (id: string, profile: Parameters<typeof commands.membersSetProfile>[1]) =>
      unwrap(commands.membersSetProfile(id, profile)),
    archive: (id: string) => unwrap(commands.membersArchive(id)),
  },
  account: {
    get: () => unwrap(commands.accountGet()),
    login: () => unwrap(commands.accountLogin()),
    cancelLogin: () => unwrap(commands.accountLoginCancel()),
    refresh: () => unwrap(commands.accountRefresh()),
    logout: () => unwrap(commands.accountLogout()),
  },
  pluralkit: {
    status: () => unwrap(commands.pkStatus()),
    link: (token: string) => unwrap(commands.pkLink(token)),
    unlink: () => unwrap(commands.pkUnlink()),
    syncNow: () => unwrap(commands.pkSyncNow()),
  },
  front: {
    current: () => unwrap(commands.frontCurrent()),
    change: (change: Parameters<typeof commands.frontChange>[0]) =>
      unwrap(commands.frontChange(change)),
    history: (from: number, to: number) => unwrap(commands.frontHistory(from, to)),
  },
  journal: {
    list: (before: string | null, author: string | null, limit: number) =>
      unwrap(commands.journalList(before, author, limit)),
    create: (input: Parameters<typeof commands.journalCreate>[0]) =>
      unwrap(commands.journalCreate(input)),
    update: (id: string, input: Parameters<typeof commands.journalUpdate>[1]) =>
      unwrap(commands.journalUpdate(id, input)),
    remove: (id: string) => unwrap(commands.journalDelete(id)),
  },
  safety: {
    get: () => unwrap(commands.safetyGet()),
    set: (plan: Parameters<typeof commands.safetySet>[0]) => unwrap(commands.safetySet(plan)),
  },
  backup: {
    export: (path: string) => unwrap(commands.backupExport(path)),
    restore: (path: string, passphrase: string) => unwrap(commands.backupRestore(path, passphrase)),
  },
  messages: {
    list: (before: string | null, limit: number) => unwrap(commands.messagesList(before, limit)),
    post: (input: Parameters<typeof commands.messagesPost>[0]) =>
      unwrap(commands.messagesPost(input)),
    setPinned: (id: string, pinned: boolean) => unwrap(commands.messagesSetPinned(id, pinned)),
    remove: (id: string) => unwrap(commands.messagesDelete(id)),
    catchUp: (memberId: string) => unwrap(commands.catchUp(memberId)),
    markRead: (memberId: string, ids: string[]) => unwrap(commands.messagesMarkRead(memberId, ids)),
  },
};
