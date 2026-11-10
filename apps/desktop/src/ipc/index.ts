import { commands, type IpcError_Serialize } from "./bindings";

export type {
  FrontChange,
  FrontSession_Serialize as FrontSession,
  Member_Serialize as Member,
  NewMember_Serialize as NewMember,
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
  },
  front: {
    current: () => unwrap(commands.frontCurrent()),
    change: (change: Parameters<typeof commands.frontChange>[0]) =>
      unwrap(commands.frontChange(change)),
    history: (from: number, to: number) => unwrap(commands.frontHistory(from, to)),
  },
};
