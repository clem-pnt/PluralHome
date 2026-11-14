// Stockage de la synchro PluralHome. Le serveur ne voit que des identifiants
// opaques (HMAC) et des blocs scellés côté client : ni noms, ni contenus, ni tables.
import { mkdirSync } from "node:fs";
import { dirname } from "node:path";
import { DatabaseSync } from "node:sqlite";

export const MAX_ACCOUNT_BYTES = 256 * 1024 * 1024;
export const PAIR_TTL = 10 * 60_000;

export class StoreError extends Error {
  constructor(status, code) {
    super(code);
    this.status = status;
    this.code = code;
  }
}

export function openStore(path, clock = Date.now) {
  if (path !== ":memory:") mkdirSync(dirname(path), { recursive: true, mode: 0o700 });
  const db = new DatabaseSync(path);
  db.exec(`
    PRAGMA journal_mode = WAL;
    PRAGMA foreign_keys = ON;
    CREATE TABLE IF NOT EXISTS accounts (
      user_id    TEXT PRIMARY KEY,
      keyset     TEXT NOT NULL,
      seq        INTEGER NOT NULL DEFAULT 0,
      bytes      INTEGER NOT NULL DEFAULT 0,
      created_at INTEGER NOT NULL
    ) STRICT;
    CREATE TABLE IF NOT EXISTS items (
      user_id TEXT NOT NULL REFERENCES accounts (user_id) ON DELETE CASCADE,
      k       TEXT NOT NULL,
      v       TEXT NOT NULL,
      seq     INTEGER NOT NULL,
      PRIMARY KEY (user_id, k)
    ) STRICT, WITHOUT ROWID;
    CREATE INDEX IF NOT EXISTS items_seq ON items (user_id, seq);
    CREATE TABLE IF NOT EXISTS pairs (
      id         TEXT PRIMARY KEY,
      user_id    TEXT NOT NULL,
      pub        TEXT NOT NULL,
      sealed     TEXT,
      expires_at INTEGER NOT NULL
    ) STRICT;
  `);
  const q = {
    account: db.prepare("SELECT keyset, seq, bytes FROM accounts WHERE user_id = ?"),
    create: db.prepare("INSERT INTO accounts (user_id, keyset, created_at) VALUES (?, ?, ?)"),
    drop: db.prepare("DELETE FROM accounts WHERE user_id = ?"),
    dropPairs: db.prepare("DELETE FROM pairs WHERE user_id = ?"),
    page: db.prepare("SELECT k, v, seq FROM items WHERE user_id = ? AND seq > ? ORDER BY seq LIMIT ?"),
    old: db.prepare("SELECT length(v) AS n FROM items WHERE user_id = ? AND k = ?"),
    put: db.prepare(`INSERT INTO items (user_id, k, v, seq) VALUES (?, ?, ?, ?)
      ON CONFLICT (user_id, k) DO UPDATE SET v = excluded.v, seq = excluded.seq`),
    bump: db.prepare("UPDATE accounts SET seq = ?, bytes = ? WHERE user_id = ?"),
    expire: db.prepare("DELETE FROM pairs WHERE expires_at < ?"),
    pairCount: db.prepare("SELECT count(*) AS n FROM pairs WHERE user_id = ?"),
    pairNew: db.prepare("INSERT INTO pairs (id, user_id, pub, expires_at) VALUES (?, ?, ?, ?)"),
    pairList: db.prepare("SELECT id, pub, expires_at AS expiresAt FROM pairs WHERE user_id = ? AND sealed IS NULL ORDER BY expires_at"),
    pairGet: db.prepare("SELECT sealed FROM pairs WHERE id = ? AND user_id = ?"),
    pairSeal: db.prepare("UPDATE pairs SET sealed = ? WHERE id = ? AND user_id = ? AND sealed IS NULL"),
    pairDel: db.prepare("DELETE FROM pairs WHERE id = ? AND user_id = ?"),
  };

  function transaction(work) {
    db.exec("BEGIN IMMEDIATE");
    try {
      const result = work();
      db.exec("COMMIT");
      return result;
    } catch (error) {
      db.exec("ROLLBACK");
      throw error;
    }
  }

  function account(user) {
    const row = q.account.get(user);
    if (!row) throw new StoreError(409, "NO_KEYSET");
    return row;
  }

  return {
    state(user) {
      const row = q.account.get(user);
      return row ? { keyset: JSON.parse(row.keyset), seq: row.seq, bytes: row.bytes } : { keyset: null, seq: 0, bytes: 0 };
    },
    createKeyset(user, keyset) {
      transaction(() => {
        if (q.account.get(user)) throw new StoreError(409, "KEYSET_EXISTS");
        q.create.run(user, JSON.stringify(keyset), clock());
      });
    },
    changes(user, since, limit) {
      account(user);
      const items = q.page.all(user, since, limit + 1);
      const more = items.length > limit;
      if (more) items.pop();
      return { items, more };
    },
    push(user, items) {
      return transaction(() => {
        let { seq, bytes } = account(user);
        for (const { k, v } of items) {
          bytes += v.length - (q.old.get(user, k)?.n ?? 0);
          q.put.run(user, k, v, ++seq);
        }
        if (bytes > MAX_ACCOUNT_BYTES) throw new StoreError(413, "QUOTA");
        q.bump.run(seq, bytes, user);
        return seq;
      });
    },
    deleteAccount(user) {
      transaction(() => {
        q.drop.run(user);
        q.dropPairs.run(user);
      });
    },
    pairCreate(user, id, pub) {
      q.expire.run(clock());
      if (q.pairCount.get(user).n >= 5) throw new StoreError(429, "TOO_MANY_PAIRINGS");
      q.pairNew.run(id, user, pub, clock() + PAIR_TTL);
    },
    pairList(user) {
      q.expire.run(clock());
      return q.pairList.all(user);
    },
    pairSeal(user, id, sealed) {
      q.expire.run(clock());
      if (q.pairSeal.run(sealed, id, user).changes !== 1) throw new StoreError(404, "PAIR_NOT_FOUND");
    },
    /** Renvoie le bloc scellé une seule fois, ou `null` tant qu'il n'est pas prêt. */
    pairTake(user, id) {
      q.expire.run(clock());
      const row = q.pairGet.get(id, user);
      if (!row) throw new StoreError(404, "PAIR_NOT_FOUND");
      if (row.sealed) q.pairDel.run(id, user);
      return row.sealed;
    },
    pairCancel(user, id) {
      q.pairDel.run(id, user);
    },
    close() {
      db.close();
    },
  };
}
