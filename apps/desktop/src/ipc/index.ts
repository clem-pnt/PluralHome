import { commands, type IpcError_Serialize, type SystemKind } from "./bindings";

export type {
  Account_Serialize as Account,
  AvatarEdit,
  Blocked_Serialize as Blocked,
  CardMember_Serialize as CardMember,
  CatchUp_Serialize as CatchUp,
  ChatGroup_Serialize as ChatGroup,
  CloudStatus_Serialize as CloudStatus,
  Contact_Serialize as Contact,
  ContactChannel_Serialize as ContactChannel,
  Entry_Serialize as Entry,
  Folder,
  Friend_Serialize as Friend,
  FriendsReport,
  FrontChange,
  FrontSession_Serialize as FrontSession,
  FrontStatus,
  GroupInput_Deserialize as GroupInput,
  IncomingRequest_Serialize as IncomingRequest,
  Invite_Serialize as Invite,
  InvitePreview_Serialize as InvitePreview,
  Member_Serialize as Member,
  Message_Serialize as Message,
  NewcomerPlan_Serialize as NewcomerPlan,
  NewEntry_Deserialize as NewEntry,
  NewMember_Serialize as NewMember,
  NewMessage,
  NewPoll_Deserialize as NewPoll,
  OutgoingRequest_Serialize as OutgoingRequest,
  PairPoll,
  PendingDevice,
  PkSystem_Serialize as PkSystem,
  PlanSection_Serialize as PlanSection,
  Poll_Serialize as Poll,
  PollOption,
  Privacy_Serialize as Privacy,
  RemoteRef,
  Requests_Serialize as Requests,
  SafetyPlan_Serialize as SafetyPlan,
  Status_Serialize as PkStatus,
  SystemKind,
  SystemProfile_Serialize as SystemProfile,
  TrustedContact_Serialize as TrustedContact,
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
  mailto: (address: string) => unwrap(commands.openMailto(address)),
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
    listArchived: () => unwrap(commands.membersListArchived()),
    unarchive: (id: string) => unwrap(commands.membersUnarchive(id)),
    remove: (id: string) => unwrap(commands.membersDelete(id)),
  },
  groups: {
    list: () => unwrap(commands.groupsList()),
    create: (input: Parameters<typeof commands.groupsCreate>[0]) =>
      unwrap(commands.groupsCreate(input)),
    update: (id: string, input: Parameters<typeof commands.groupsUpdate>[1]) =>
      unwrap(commands.groupsUpdate(id, input)),
    remove: (id: string) => unwrap(commands.groupsDelete(id)),
  },
  system: {
    get: () => unwrap(commands.systemGet()),
    set: (input: Parameters<typeof commands.systemSet>[0]) => unwrap(commands.systemSet(input)),
    setKind: (kind: SystemKind) => unwrap(commands.systemSetKind(kind)),
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
    signedOut: () => unwrap(commands.accountSignedOut()),
    continueGuest: () => unwrap(commands.accountContinueGuest()),
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
    first: () => unwrap(commands.frontFirst()),
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
  newcomer: {
    get: () => unwrap(commands.newcomerGet()),
    set: (plan: Parameters<typeof commands.newcomerSet>[0]) => unwrap(commands.newcomerSet(plan)),
  },
  trusted: {
    get: () => unwrap(commands.trustedGet()),
    set: (contact: Parameters<typeof commands.trustedSet>[0]) =>
      unwrap(commands.trustedSet(contact)),
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
  friends: {
    list: () => unwrap(commands.friendsList()),
    privacy: () => unwrap(commands.friendsPrivacyGet()),
    setPrivacy: (p: Parameters<typeof commands.friendsPrivacySet>[0]) =>
      unwrap(commands.friendsPrivacySet(p)),
    setPref: (
      id: string,
      nickname: string | null,
      privacy: Parameters<typeof commands.friendsPrivacySet>[0] | null,
    ) => unwrap(commands.friendsSetPref(id, nickname, privacy)),
    settings: () => unwrap(commands.friendsSettings()),
    setOpen: (open: boolean) => unwrap(commands.friendsSetOpen(open)),
    createInvite: (ttl: number | null, once: boolean) =>
      unwrap(commands.friendsInviteCreate(ttl, once)),
    invites: () => unwrap(commands.friendsInvites()),
    revokeInvite: (id: string) => unwrap(commands.friendsInviteRevoke(id)),
    preview: (text: string) => unwrap(commands.friendsPreview(text)),
    sendRequest: (text: string) => unwrap(commands.friendsRequestSend(text)),
    requests: () => unwrap(commands.friendsRequests()),
    accept: (id: string) => unwrap(commands.friendsAccept(id)),
    dropRequest: (id: string) => unwrap(commands.friendsDropRequest(id)),
    blockRequest: (id: string, name: string | null) =>
      unwrap(commands.friendsBlockRequest(id, name)),
    unfriend: (id: string) => unwrap(commands.friendsUnfriend(id)),
    block: (id: string) => unwrap(commands.friendsBlock(id)),
    blocks: () => unwrap(commands.friendsBlocks()),
    unblock: (id: string) => unwrap(commands.friendsUnblock(id)),
    takeOver: () => unwrap(commands.friendsTakeOver()),
    syncNow: () => unwrap(commands.friendsSyncNow()),
  },
};
