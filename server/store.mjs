// Stockage de la synchro PluralHome. Le serveur ne voit que des identifiants
// opaques (HMAC) et des blocs scellés côté client : ni noms, ni contenus, ni tables.
import { randomUUID } from "node:crypto";
import { mkdirSync } from "node:fs";
import { dirname } from "node:path";
import { DatabaseSync } from "node:sqlite";

export const MAX_ACCOUNT_BYTES = 256 * 1024 * 1024;
export const PAIR_TTL = 10 * 60_000;
export const MAX_INVITES = 20;
export const MAX_PENDING = 50;
export const MAX_FRIENDS = 300;
export const MAX_MAIL = 5000;
export const MAIL_TTL = 60 * 24 * 3600_000;

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

    -- Amis. Les comptes ne sont jamais exposés : les clients ne voient que des
    -- identifiants aléatoires et la clé publique X25519 de chaque système.
    CREATE TABLE IF NOT EXISTS identities (
      user_id    TEXT PRIMARY KEY,
      pub        TEXT NOT NULL UNIQUE,
      open       INTEGER NOT NULL DEFAULT 1,
      created_at INTEGER NOT NULL
    ) STRICT;
    CREATE TABLE IF NOT EXISTS invites (
      id         TEXT PRIMARY KEY,
      user_id    TEXT NOT NULL,
      card       TEXT NOT NULL,
      once       INTEGER NOT NULL,
      expires_at INTEGER,
      created_at INTEGER NOT NULL
    ) STRICT;
    CREATE INDEX IF NOT EXISTS invites_user ON invites (user_id);
    CREATE TABLE IF NOT EXISTS requests (
      id         TEXT PRIMARY KEY,
      from_user  TEXT NOT NULL,
      to_user    TEXT NOT NULL,
      card       TEXT NOT NULL,
      created_at INTEGER NOT NULL,
      UNIQUE (from_user, to_user)
    ) STRICT;
    CREATE INDEX IF NOT EXISTS requests_to ON requests (to_user);
    -- a < b ; card_a : fiche écrite par a pour b (scellée pour b).
    CREATE TABLE IF NOT EXISTS friendships (
      id         TEXT PRIMARY KEY,
      a          TEXT NOT NULL,
      b          TEXT NOT NULL,
      card_a     TEXT NOT NULL,
      card_b     TEXT NOT NULL,
      at_a       INTEGER NOT NULL,
      at_b       INTEGER NOT NULL,
      created_at INTEGER NOT NULL,
      UNIQUE (a, b)
    ) STRICT;
    CREATE INDEX IF NOT EXISTS friendships_b ON friendships (b);
    CREATE TABLE IF NOT EXISTS blocks (
      id         TEXT PRIMARY KEY,
      user_id    TEXT NOT NULL,
      blocked    TEXT NOT NULL,
      created_at INTEGER NOT NULL,
      UNIQUE (user_id, blocked)
    ) STRICT;
    CREATE TABLE IF NOT EXISTS mail (
      seq        INTEGER PRIMARY KEY AUTOINCREMENT,
      to_user    TEXT NOT NULL,
      friendship TEXT NOT NULL,
      sealed     TEXT NOT NULL,
      created_at INTEGER NOT NULL
    ) STRICT;
    CREATE INDEX IF NOT EXISTS mail_to ON mail (to_user, seq);
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

    identity: db.prepare("SELECT pub, open FROM identities WHERE user_id = ?"),
    identityPut: db.prepare(`INSERT INTO identities (user_id, pub, created_at) VALUES (?, ?, ?)
      ON CONFLICT (user_id) DO UPDATE SET pub = excluded.pub`),
    identityOpen: db.prepare("UPDATE identities SET open = ? WHERE user_id = ?"),
    identityDrop: db.prepare("DELETE FROM identities WHERE user_id = ?"),
    pubOf: db.prepare("SELECT pub FROM identities WHERE user_id = ?"),

    inviteExpire: db.prepare("DELETE FROM invites WHERE expires_at IS NOT NULL AND expires_at < ?"),
    inviteCount: db.prepare("SELECT count(*) AS n FROM invites WHERE user_id = ?"),
    inviteNew: db.prepare("INSERT INTO invites (id, user_id, card, once, expires_at, created_at) VALUES (?, ?, ?, ?, ?, ?)"),
    inviteList: db.prepare(`SELECT id, once, expires_at AS expiresAt, created_at AS createdAt
      FROM invites WHERE user_id = ? ORDER BY created_at DESC`),
    inviteGet: db.prepare(`SELECT i.user_id, i.card, i.once, d.open FROM invites i
      JOIN identities d ON d.user_id = i.user_id WHERE i.id = ?`),
    inviteDel: db.prepare("DELETE FROM invites WHERE id = ? AND user_id = ?"),
    inviteDrop: db.prepare("DELETE FROM invites WHERE user_id = ?"),

    reqIncoming: db.prepare(`SELECT r.id, d.pub, r.card, r.created_at AS createdAt FROM requests r
      JOIN identities d ON d.user_id = r.from_user WHERE r.to_user = ? ORDER BY r.created_at`),
    reqOutgoing: db.prepare("SELECT id, created_at AS createdAt FROM requests WHERE from_user = ? ORDER BY created_at"),
    reqCountTo: db.prepare("SELECT count(*) AS n FROM requests WHERE to_user = ?"),
    reqCountFrom: db.prepare("SELECT count(*) AS n FROM requests WHERE from_user = ?"),
    reqPair: db.prepare("SELECT id FROM requests WHERE from_user = ? AND to_user = ?"),
    reqNew: db.prepare("INSERT INTO requests (id, from_user, to_user, card, created_at) VALUES (?, ?, ?, ?, ?)"),
    reqCard: db.prepare("UPDATE requests SET card = ?, created_at = ? WHERE id = ?"),
    reqGet: db.prepare("SELECT from_user, to_user, card FROM requests WHERE id = ?"),
    reqDel: db.prepare("DELETE FROM requests WHERE id = ?"),
    reqDropBetween: db.prepare("DELETE FROM requests WHERE (from_user = ?1 AND to_user = ?2) OR (from_user = ?2 AND to_user = ?1)"),
    reqDrop: db.prepare("DELETE FROM requests WHERE from_user = ?1 OR to_user = ?1"),

    friendCount: db.prepare("SELECT count(*) AS n FROM friendships WHERE a = ?1 OR b = ?1"),
    friendBetween: db.prepare("SELECT id FROM friendships WHERE a = ? AND b = ?"),
    friendNew: db.prepare(`INSERT INTO friendships (id, a, b, card_a, card_b, at_a, at_b, created_at)
      VALUES (?, ?, ?, ?, ?, ?, ?, ?)`),
    friendList: db.prepare(`SELECT f.id, d.pub, CASE WHEN f.a = ?1 THEN f.card_b ELSE f.card_a END AS card,
        CASE WHEN f.a = ?1 THEN f.at_b ELSE f.at_a END AS cardAt, f.created_at AS createdAt
      FROM friendships f JOIN identities d ON d.user_id = CASE WHEN f.a = ?1 THEN f.b ELSE f.a END
      WHERE f.a = ?1 OR f.b = ?1 ORDER BY f.created_at`),
    friendGet: db.prepare("SELECT a, b FROM friendships WHERE id = ?"),
    friendCardA: db.prepare("UPDATE friendships SET card_a = ?, at_a = ? WHERE id = ?"),
    friendCardB: db.prepare("UPDATE friendships SET card_b = ?, at_b = ? WHERE id = ?"),
    friendDel: db.prepare("DELETE FROM friendships WHERE id = ?"),
    friendIds: db.prepare("SELECT id FROM friendships WHERE a = ?1 OR b = ?1"),

    blockNew: db.prepare("INSERT OR IGNORE INTO blocks (id, user_id, blocked, created_at) VALUES (?, ?, ?, ?)"),
    blockOf: db.prepare("SELECT id FROM blocks WHERE user_id = ? AND blocked = ?"),
    blockList: db.prepare("SELECT id, created_at AS createdAt FROM blocks WHERE user_id = ? ORDER BY created_at"),
    blockDel: db.prepare("DELETE FROM blocks WHERE id = ? AND user_id = ?"),
    blockDrop: db.prepare("DELETE FROM blocks WHERE user_id = ?1 OR blocked = ?1"),

    mailExpire: db.prepare("DELETE FROM mail WHERE created_at < ?"),
    mailCount: db.prepare("SELECT count(*) AS n FROM mail WHERE to_user = ?"),
    mailNew: db.prepare("INSERT INTO mail (to_user, friendship, sealed, created_at) VALUES (?, ?, ?, ?)"),
    mailPage: db.prepare(`SELECT seq AS s, friendship AS friend, sealed AS v FROM mail
      WHERE to_user = ? AND seq > ? ORDER BY seq LIMIT ?`),
    mailAck: db.prepare("DELETE FROM mail WHERE to_user = ? AND seq <= ?"),
    mailDropFriend: db.prepare("DELETE FROM mail WHERE friendship = ?"),
    mailDrop: db.prepare("DELETE FROM mail WHERE to_user = ?"),
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

  const ordered = (x, y) => (x < y ? [x, y] : [y, x]);

  /** Membre de l'amitié ; renvoie l'autre compte. */
  function friendship(user, id) {
    const row = q.friendGet.get(id);
    if (!row || (row.a !== user && row.b !== user)) throw new StoreError(404, "FRIEND_NOT_FOUND");
    return { ...row, other: row.a === user ? row.b : row.a };
  }

  function removeFriendship(id) {
    q.friendDel.run(id);
    q.mailDropFriend.run(id);
  }

  /** Efface tout ce qui touche aux amis pour ce compte. */
  function dropFriends(user) {
    for (const { id } of q.friendIds.all(user)) removeFriendship(id);
    q.reqDrop.run(user);
    q.inviteDrop.run(user);
    q.mailDrop.run(user);
  }

  function block(user, other) {
    q.blockNew.run(randomUUID(), user, other, clock());
    const [a, b] = ordered(user, other);
    const f = q.friendBetween.get(a, b);
    if (f) removeFriendship(f.id);
    q.reqDropBetween.run(user, other);
    return q.blockOf.get(user, other).id;
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
        dropFriends(user);
        q.blockDrop.run(user);
        q.identityDrop.run(user);
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

    identity(user) {
      const row = q.identity.get(user);
      return { pub: row?.pub ?? null, open: row ? row.open === 1 : true };
    },
    /** Pose la clé publique du système. La remplacer coupe tous les liens d'amitié. */
    setIdentity(user, pub, replace) {
      transaction(() => {
        const row = q.identity.get(user);
        if (row?.pub === pub) return;
        if (row && !replace) throw new StoreError(409, "IDENTITY_EXISTS");
        if (row) dropFriends(user);
        try {
          q.identityPut.run(user, pub, clock());
        } catch {
          throw new StoreError(409, "PUB_TAKEN");
        }
      });
    },
    setOpen(user, open) {
      if (q.identityOpen.run(open ? 1 : 0, user).changes !== 1) throw new StoreError(409, "NO_IDENTITY");
    },

    inviteCreate(user, card, once, ttl) {
      return transaction(() => {
        if (!q.identity.get(user)) throw new StoreError(409, "NO_IDENTITY");
        q.inviteExpire.run(clock());
        if (q.inviteCount.get(user).n >= MAX_INVITES) throw new StoreError(429, "TOO_MANY_INVITES");
        const id = randomUUID();
        q.inviteNew.run(id, user, card, once ? 1 : 0, ttl == null ? null : clock() + ttl, clock());
        return id;
      });
    },
    inviteList(user) {
      q.inviteExpire.run(clock());
      return q.inviteList.all(user).map((i) => ({ ...i, once: i.once === 1 }));
    },
    inviteRevoke(user, id) {
      q.inviteDel.run(id, user);
    },
    /** Fiche scellée d'une invitation valide (le secret pour l'ouvrir n'est jamais envoyé ici). */
    inviteRead(id) {
      q.inviteExpire.run(clock());
      const row = q.inviteGet.get(id);
      if (!row || row.open !== 1) throw new StoreError(404, "INVITE_INVALID");
      return row.card;
    },

    /** Envoie une demande grâce à une invitation. Un blocage la fait disparaître sans bruit. */
    requestSend(user, inviteId, card) {
      return transaction(() => {
        if (!q.identity.get(user)) throw new StoreError(409, "NO_IDENTITY");
        q.inviteExpire.run(clock());
        const inv = q.inviteGet.get(inviteId);
        if (!inv || inv.open !== 1) throw new StoreError(404, "INVITE_INVALID");
        const to = inv.user_id;
        if (to === user) throw new StoreError(409, "SELF");
        const [a, b] = ordered(user, to);
        if (q.friendBetween.get(a, b)) throw new StoreError(409, "ALREADY_FRIENDS");
        if (q.blockOf.get(user, to)) throw new StoreError(409, "BLOCKED");
        if (inv.once === 1) q.inviteDel.run(inviteId, to);
        // Bloqué par le destinataire : on répond comme si de rien n'était.
        if (q.blockOf.get(to, user)) return randomUUID();
        const existing = q.reqPair.get(user, to);
        if (existing) {
          q.reqCard.run(card, clock(), existing.id);
          return existing.id;
        }
        if (q.reqCountTo.get(to).n >= MAX_PENDING) throw new StoreError(429, "INBOX_FULL");
        if (q.reqCountFrom.get(user).n >= MAX_PENDING) throw new StoreError(429, "TOO_MANY_REQUESTS");
        const id = randomUUID();
        q.reqNew.run(id, user, to, card, clock());
        return id;
      });
    },
    requests(user) {
      return { incoming: q.reqIncoming.all(user), outgoing: q.reqOutgoing.all(user) };
    },
    requestAccept(user, id, card) {
      return transaction(() => {
        const req = q.reqGet.get(id);
        if (!req || req.to_user !== user) throw new StoreError(404, "REQUEST_NOT_FOUND");
        if (q.friendCount.get(user).n >= MAX_FRIENDS) throw new StoreError(429, "TOO_MANY_FRIENDS");
        q.reqDropBetween.run(req.from_user, user);
        const [a, b] = ordered(req.from_user, user);
        const existing = q.friendBetween.get(a, b);
        if (existing) return existing.id;
        const fid = randomUUID();
        const fromIsA = a === req.from_user;
        const now = clock();
        q.friendNew.run(fid, a, b, fromIsA ? req.card : card, fromIsA ? card : req.card, now, now, now);
        return fid;
      });
    },
    /** Refus (destinataire) ou annulation (expéditeur). */
    requestDrop(user, id) {
      const req = q.reqGet.get(id);
      if (req && (req.to_user === user || req.from_user === user)) q.reqDel.run(id);
    },
    requestBlock(user, id) {
      return transaction(() => {
        const req = q.reqGet.get(id);
        if (!req || req.to_user !== user) throw new StoreError(404, "REQUEST_NOT_FOUND");
        return block(user, req.from_user);
      });
    },

    friends(user) {
      return q.friendList.all(user);
    },
    setCard(user, id, card) {
      const f = friendship(user, id);
      (f.a === user ? q.friendCardA : q.friendCardB).run(card, clock(), id);
    },
    unfriend(user, id) {
      transaction(() => {
        friendship(user, id);
        removeFriendship(id);
      });
    },
    blockFriend(user, id) {
      return transaction(() => block(user, friendship(user, id).other));
    },
    blocks(user) {
      return q.blockList.all(user);
    },
    unblock(user, id) {
      q.blockDel.run(id, user);
    },

    mailSend(user, id, items) {
      transaction(() => {
        const { other } = friendship(user, id);
        q.mailExpire.run(clock() - MAIL_TTL);
        if (q.mailCount.get(other).n + items.length > MAX_MAIL) throw new StoreError(429, "MAILBOX_FULL");
        for (const v of items) q.mailNew.run(other, id, v, clock());
      });
    },
    mail(user, after, limit) {
      const items = q.mailPage.all(user, after, limit + 1);
      const more = items.length > limit;
      if (more) items.pop();
      return { items, more };
    },
    mailAck(user, upTo) {
      q.mailAck.run(user, upTo);
    },

    close() {
      db.close();
    },
  };
}
