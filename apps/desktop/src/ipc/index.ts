import { commands, type IpcError_Serialize } from "./bindings";

export type {
  Account_Serialize as Account,
  AvatarEdit,
  CatchUp_Serialize as CatchUp,
  CloudStatus_Serialize as CloudStatus,
  Contact_Serialize as Contact,
  Entry_Serialize as Entry,
  Folder,
  FrontChange,
  FrontSession_Serialize as FrontSession,
  FrontStatus,
  Member_Serialize as Member,
  Message_Serialize as Message,
  NewEntry_Deserialize as NewEntry,
  NewMember_Serialize as NewMember,
  NewMessage,
  NewPoll_Deserialize as NewPoll,
  PairPoll,
  PendingDevice,
  PkSystem_Serialize as PkSystem,
  Poll_Serialize as Poll,
  PollOption,
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
    open: () => unwrap(commands.vaultOpen()),
    create: () => unwrap(commands.vaultCreate()),
    startOver: () => unwrap(commands.vaultStartOver()),
    upgrade: (passphrase: string) => unwrap(commands.vaultUpgrade(passphrase)),
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
  roles: {
    list: () => unwrap(commands.rolesList()),
    add: (name: string) => unwrap(commands.rolesAdd(name)),
    rename: (from: string, to: string) => unwrap(commands.rolesRename(from, to)),
    remove: (name: string) => unwrap(commands.rolesRemove(name)),
  },
  folders: {
    list: () => unwrap(commands.foldersList()),
    create: (name: string, parent: string | null = null) =>
      unwrap(commands.foldersCreate(name, parent)),
    move: (id: string, parent: string | null) => unwrap(commands.foldersMove(id, parent)),
    rename: (id: string, name: string) => unwrap(commands.foldersRename(id, name)),
    remove: (id: string) => unwrap(commands.foldersRemove(id)),
    setMembers: (id: string, members: string[]) => unwrap(commands.foldersSetMembers(id, members)),
    setForMember: (member: string, folders: string[]) =>
      unwrap(commands.foldersSetForMember(member, folders)),
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
    setSyncDescription: (enabled: boolean) => unwrap(commands.pkSetSyncDescription(enabled)),
  },
  front: {
    current: () => unwrap(commands.frontCurrent()),
    change: (change: Parameters<typeof commands.frontChange>[0]) =>
      unwrap(commands.frontChange(change)),
    history: (from: number, to: number) => unwrap(commands.frontHistory(from, to)),
    quick: () => unwrap(commands.frontQuick()),
    setFavorites: (members: string[]) => unwrap(commands.frontSetFavorites(members)),
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
  cloud: {
    status: () => unwrap(commands.cloudStatus()),
    probe: () => unwrap(commands.cloudProbe()),
    enable: () => unwrap(commands.cloudEnable()),
    joinRecovery: (recovery: string) => unwrap(commands.cloudJoinRecovery(recovery)),
    pairRequest: () => unwrap(commands.cloudPairRequest()),
    pairPoll: () => unwrap(commands.cloudPairPoll()),
    pairCancel: () => unwrap(commands.cloudPairCancel()),
    pairRequests: () => unwrap(commands.cloudPairRequests()),
    pairApprove: (id: string) => unwrap(commands.cloudPairApprove(id)),
    pairReject: (id: string) => unwrap(commands.cloudPairReject(id)),
    disable: () => unwrap(commands.cloudDisable()),
    remove: () => unwrap(commands.cloudDelete()),
    syncNow: () => unwrap(commands.cloudSyncNow()),
  },
  polls: {
    list: () => unwrap(commands.pollsList()),
    create: (input: Parameters<typeof commands.pollsCreate>[0]) =>
      unwrap(commands.pollsCreate(input)),
    vote: (id: string, voters: string[], options: string[]) =>
      unwrap(commands.pollsVote(id, voters, options)),
    setClosed: (id: string, closed: boolean) => unwrap(commands.pollsSetClosed(id, closed)),
    remove: (id: string) => unwrap(commands.pollsDelete(id)),
  },
  messages: {
    list: (before: string | null, limit: number, unreadOnly: boolean) =>
      unwrap(commands.messagesList(before, limit, unreadOnly)),
    post: (input: Parameters<typeof commands.messagesPost>[0]) =>
      unwrap(commands.messagesPost(input)),
    update: (id: string, input: Parameters<typeof commands.messagesPost>[0]) =>
      unwrap(commands.messagesUpdate(id, input)),
    markReadBy: (id: string, members: string[]) => unwrap(commands.messagesMarkReadBy(id, members)),
    setPinned: (id: string, pinned: boolean) => unwrap(commands.messagesSetPinned(id, pinned)),
    remove: (id: string) => unwrap(commands.messagesDelete(id)),
    catchUp: (memberId: string) => unwrap(commands.catchUp(memberId)),
    markRead: (memberId: string, ids: string[]) => unwrap(commands.messagesMarkRead(memberId, ids)),
  },
};
