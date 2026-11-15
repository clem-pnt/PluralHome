import assert from "node:assert/strict";
import { createServer } from "node:http";
import { after, before, test } from "node:test";
import { createHandler } from "../app.mjs";
import { openStore } from "../store.mjs";

let now = 1_000_000;
const clock = () => now;
const users = { "token-alice-0000000": "u-alice", "token-bob-000000000": "u-bob", "token-carol-0000000": "u-carol" };
const A = "token-alice-0000000";
const B = "token-bob-000000000";
const C = "token-carol-0000000";
const pub = (c) => c.repeat(64);

let server;
let base;
before(async () => {
  const store = openStore(":memory:", clock);
  const verify = async (t) => users[t];
  server = createServer(createHandler({ store, verify, clock }));
  await new Promise((r) => server.listen(0, "127.0.0.1", r));
  base = `http://127.0.0.1:${server.address().port}`;
});
after(() => server.close());

async function call(token, method, path, body) {
  const res = await fetch(base + path, {
    method,
    headers: { authorization: `Bearer ${token}`, "content-type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  return { status: res.status, body: await res.json() };
}

test("identité : posée une fois, remplacement explicite", async () => {
  assert.deepEqual((await call(A, "GET", "/v1/friends/identity")).body, { pub: null, open: true });
  assert.equal((await call(A, "PUT", "/v1/friends/identity", { pub: "zz" })).status, 400);
  assert.equal((await call(A, "PUT", "/v1/friends/identity", { pub: pub("a") })).status, 200);
  assert.equal((await call(A, "PUT", "/v1/friends/identity", { pub: pub("a") })).status, 200);
  assert.equal((await call(A, "PUT", "/v1/friends/identity", { pub: pub("9") })).body.error, "IDENTITY_EXISTS");
  assert.equal((await call(B, "PUT", "/v1/friends/identity", { pub: pub("a") })).body.error, "PUB_TAKEN");
  assert.equal((await call(B, "PUT", "/v1/friends/identity", { pub: pub("b") })).status, 200);
  assert.equal((await call(C, "POST", "/v1/friends/invites", { card: "Yw==" })).body.error, "NO_IDENTITY");
  assert.equal((await call(C, "PUT", "/v1/friends/identity", { pub: pub("c") })).status, 200);
});

test("invitation, demande, acceptation, fiches et courrier", async () => {
  const inv = await call(A, "POST", "/v1/friends/invites", { card: "aW52", ttl: 3600_000, once: true });
  assert.equal(inv.status, 200);
  const id = inv.body.id;
  assert.equal((await call(A, "GET", "/v1/friends/invites")).body.invites.length, 1);
  assert.deepEqual((await call(B, "GET", `/v1/friends/invites/${id}`)).body, { card: "aW52" });
  assert.equal((await call(A, "POST", "/v1/friends/requests", { invite: id, card: "eA==" })).body.error, "SELF");

  const req = await call(B, "POST", "/v1/friends/requests", { invite: id, card: "Ym9i" });
  assert.equal(req.status, 200);
  // Invitation à usage unique : consommée.
  assert.equal((await call(C, "GET", `/v1/friends/invites/${id}`)).status, 404);

  const inbox = (await call(A, "GET", "/v1/friends/requests")).body;
  assert.equal(inbox.incoming.length, 1);
  assert.equal(inbox.incoming[0].pub, pub("b"));
  assert.equal(inbox.incoming[0].card, "Ym9i");
  assert.equal(JSON.stringify(inbox).includes("u-bob"), false);
  assert.equal((await call(B, "GET", "/v1/friends/requests")).body.outgoing[0].id, req.body.id);

  assert.equal((await call(B, "POST", `/v1/friends/requests/${req.body.id}/accept`, { card: "eA==" })).status, 404);
  const acc = await call(A, "POST", `/v1/friends/requests/${req.body.id}/accept`, { card: "YWxp" });
  assert.equal(acc.status, 200);
  const fid = acc.body.id;

  const fa = (await call(A, "GET", "/v1/friends")).body.friends;
  const fb = (await call(B, "GET", "/v1/friends")).body.friends;
  assert.equal(fa[0].pub, pub("b"));
  assert.equal(fa[0].card, "Ym9i");
  assert.equal(fb[0].pub, pub("a"));
  assert.equal(fb[0].card, "YWxp");
  assert.equal((await call(A, "GET", "/v1/friends/requests")).body.incoming.length, 0);

  now += 1000;
  assert.equal((await call(B, "PUT", `/v1/friends/${fid}/card`, { card: "bmV3" })).status, 200);
  const updated = (await call(A, "GET", "/v1/friends")).body.friends[0];
  assert.equal(updated.card, "bmV3");
  assert.equal(updated.cardAt, now);
  assert.equal((await call(C, "PUT", `/v1/friends/${fid}/card`, { card: "eA==" })).status, 404);

  assert.equal((await call(A, "POST", `/v1/friends/${fid}/mail`, { items: ["bTE=", "bTI="] })).status, 200);
  assert.equal((await call(C, "POST", `/v1/friends/${fid}/mail`, { items: ["eA=="] })).status, 404);
  const mail = (await call(B, "GET", "/v1/mail?after=0")).body;
  assert.deepEqual(
    mail.items.map((m) => [m.friend, m.v]),
    [
      [fid, "bTE="],
      [fid, "bTI="],
    ],
  );
  assert.equal((await call(A, "GET", "/v1/mail")).body.items.length, 0);
  await call(B, "POST", "/v1/mail/ack", { upTo: mail.items[0].s });
  assert.equal((await call(B, "GET", "/v1/mail")).body.items.length, 1);

  // Nouvelle demande alors que déjà amis : refusée.
  const inv2 = (await call(A, "POST", "/v1/friends/invites", { card: "aW52", ttl: null })).body.id;
  assert.equal(
    (await call(B, "POST", "/v1/friends/requests", { invite: inv2, card: "eA==" })).body.error,
    "ALREADY_FRIENDS",
  );

  assert.equal((await call(B, "DELETE", `/v1/friends/${fid}`)).status, 200);
  assert.equal((await call(A, "GET", "/v1/friends")).body.friends.length, 0);
  assert.equal((await call(B, "GET", "/v1/mail")).body.items.length, 0);
});

test("expiration, fermeture, refus et blocage silencieux", async () => {
  const inv = (await call(C, "POST", "/v1/friends/invites", { card: "Yw==", ttl: 600_000 })).body.id;
  assert.equal((await call(C, "POST", "/v1/friends/invites", { card: "Yw==", ttl: 5 })).body.error, "BAD_TTL");
  now += 600_001;
  assert.equal((await call(A, "GET", `/v1/friends/invites/${inv}`)).status, 404);

  const open = (await call(C, "POST", "/v1/friends/invites", { card: "Yw==", ttl: null, once: false })).body.id;
  await call(C, "PUT", "/v1/friends/settings", { open: false });
  assert.equal((await call(A, "POST", "/v1/friends/requests", { invite: open, card: "eA==" })).status, 404);
  await call(C, "PUT", "/v1/friends/settings", { open: true });

  const r1 = (await call(A, "POST", "/v1/friends/requests", { invite: open, card: "eA==" })).body.id;
  // Renvoyer remplace la demande existante.
  assert.equal((await call(A, "POST", "/v1/friends/requests", { invite: open, card: "eQ==" })).body.id, r1);
  assert.equal((await call(C, "GET", "/v1/friends/requests")).body.incoming[0].card, "eQ==");
  await call(C, "DELETE", `/v1/friends/requests/${r1}`);
  assert.equal((await call(A, "GET", "/v1/friends/requests")).body.outgoing.length, 0);

  const r2 = (await call(A, "POST", "/v1/friends/requests", { invite: open, card: "eA==" })).body.id;
  const blocked = await call(C, "POST", `/v1/friends/requests/${r2}/block`);
  assert.equal(blocked.status, 200);
  // Alice ne voit pas qu'elle est bloquée.
  const r3 = await call(A, "POST", "/v1/friends/requests", { invite: open, card: "eA==" });
  assert.equal(r3.status, 200);
  assert.equal((await call(C, "GET", "/v1/friends/requests")).body.incoming.length, 0);

  const blocks = (await call(C, "GET", "/v1/friends/blocks")).body.blocks;
  assert.equal(blocks.length, 1);
  assert.equal(JSON.stringify(blocks).includes("u-alice"), false);
  await call(C, "DELETE", `/v1/friends/blocks/${blocks[0].id}`);
  assert.equal((await call(A, "POST", "/v1/friends/requests", { invite: open, card: "eA==" })).status, 200);
  assert.equal((await call(C, "GET", "/v1/friends/requests")).body.incoming.length, 1);
});

test("changer d'identité ou supprimer le compte efface les liens", async () => {
  const inv = (await call(B, "POST", "/v1/friends/invites", { card: "Yg==", ttl: null })).body.id;
  const r = (await call(C, "POST", "/v1/friends/requests", { invite: inv, card: "eA==" })).body.id;
  await call(B, "POST", `/v1/friends/requests/${r}/accept`, { card: "eA==" });
  assert.equal((await call(C, "GET", "/v1/friends")).body.friends.length, 1);
  assert.equal((await call(B, "PUT", "/v1/friends/identity", { pub: pub("d"), replace: true })).status, 200);
  assert.equal((await call(C, "GET", "/v1/friends")).body.friends.length, 0);
  assert.equal((await call(B, "GET", "/v1/friends/invites")).body.invites.length, 0);

  await call(C, "DELETE", "/v1/account");
  assert.deepEqual((await call(C, "GET", "/v1/friends/identity")).body, { pub: null, open: true });
  assert.equal((await call(A, "GET", "/v1/friends/requests")).body.outgoing.length, 0);
});
