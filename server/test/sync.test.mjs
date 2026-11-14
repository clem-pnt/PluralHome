import assert from "node:assert/strict";
import { createServer } from "node:http";
import { after, before, test } from "node:test";
import { createHandler, ssoVerifier } from "../app.mjs";
import { openStore } from "../store.mjs";

const TOKENS = { "token-alice-0000000": "u-alice", "token-bob-000000000": "u-bob" };
let now = 1_000_000;
const clock = () => now;
let ssoCalls = 0;
const fakeFetch = async (_url, init) => {
  ssoCalls++;
  assert.equal(new URLSearchParams(init.body).get("client_id"), "pluralhome");
  const user = TOKENS[init.headers.authorization.slice(7)];
  return user
    ? new Response(JSON.stringify({ user: { id: user } }), { status: 200 })
    : new Response("{}", { status: 401 });
};

let server;
let base;
before(async () => {
  const store = openStore(":memory:", clock);
  const verify = ssoVerifier({ origin: "http://sso", clientId: "pluralhome", fetchImpl: fakeFetch, clock });
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
const A = "token-alice-0000000";
const B = "token-bob-000000000";
const k = (n) => n.toString(16).padStart(64, "0");

test("refuse sans jeton valide", async () => {
  assert.equal((await call("nope-nope-nope-nope", "GET", "/v1/state")).status, 401);
  const res = await fetch(`${base}/v1/state`);
  assert.equal(res.status, 401);
});

test("cycle complet : jeu de clés, envoi, pagination, isolation", async () => {
  assert.deepEqual((await call(A, "GET", "/v1/state")).body, { keyset: null, seq: 0, bytes: 0 });
  assert.equal((await call(A, "POST", "/v1/changes", { items: [{ k: k(1), v: "AAAA" }] })).status, 409);
  assert.equal((await call(A, "PUT", "/v1/keyset", { id: "ks-12345678", recovery: "cmVjb3Zlcnk=" })).status, 200);
  assert.equal((await call(A, "PUT", "/v1/keyset", { id: "ks-other-1234", recovery: "eA==" })).status, 409);

  const items = Array.from({ length: 5 }, (_, i) => ({ k: k(i), v: "QUJD" }));
  assert.deepEqual((await call(A, "POST", "/v1/changes", { items })).body, { seq: 5 });
  // Réécrire une clé la renvoie en fin de flux.
  assert.deepEqual((await call(A, "POST", "/v1/changes", { items: [{ k: k(0), v: "WFla" }] })).body, { seq: 6 });

  const p1 = (await call(A, "GET", "/v1/changes?since=0&limit=3")).body;
  assert.deepEqual(p1.items.map((i) => i.s), [2, 3, 4]);
  assert.equal(p1.more, true);
  const p2 = (await call(A, "GET", `/v1/changes?since=${p1.seq}&limit=3`)).body;
  assert.deepEqual(p2.items.map((i) => [i.k, i.v, i.s]), [[k(4), "QUJD", 5], [k(0), "WFla", 6]]);
  assert.equal(p2.more, false);

  const state = (await call(A, "GET", "/v1/state")).body;
  assert.equal(state.seq, 6);
  assert.equal(state.keyset.id, "ks-12345678");
  assert.equal(state.bytes, 20);

  assert.deepEqual((await call(B, "GET", "/v1/state")).body.keyset, null);
  assert.equal((await call(B, "GET", "/v1/changes?since=0")).status, 409);
});

test("valide les entrées", async () => {
  assert.equal((await call(A, "POST", "/v1/changes", { items: [{ k: "zz", v: "AAAA" }] })).status, 400);
  assert.equal((await call(A, "POST", "/v1/changes", { items: [{ k: k(1), v: "pas du base64 !" }] })).status, 400);
  assert.equal((await call(A, "GET", "/v1/changes?since=-1")).status, 400);
  assert.equal((await call(A, "PUT", "/v1/keyset", { id: "x", recovery: "eA==" })).status, 400);
  assert.equal((await call(A, "GET", "/v1/nope")).status, 404);
});

test("appairage : demande, approbation, remise unique, expiration", async () => {
  const pub = "ab".repeat(32);
  const { id } = (await call(A, "POST", "/v1/pair", { pub })).body;
  assert.equal((await call(B, "GET", "/v1/pair")).body.requests.length, 0);
  assert.deepEqual((await call(A, "GET", `/v1/pair/${id}`)).body, { sealed: null });
  const list = (await call(A, "GET", "/v1/pair")).body.requests;
  assert.deepEqual(list.map((r) => [r.id, r.pub]), [[id, pub]]);
  assert.equal((await call(B, "PUT", `/v1/pair/${id}`, { sealed: "c2VhbGVk" })).status, 404);
  assert.equal((await call(A, "PUT", `/v1/pair/${id}`, { sealed: "c2VhbGVk" })).status, 200);
  assert.equal((await call(A, "GET", "/v1/pair")).body.requests.length, 0);
  assert.deepEqual((await call(A, "GET", `/v1/pair/${id}`)).body, { sealed: "c2VhbGVk" });
  assert.equal((await call(A, "GET", `/v1/pair/${id}`)).status, 404);

  const late = (await call(A, "POST", "/v1/pair", { pub })).body.id;
  now += 11 * 60_000;
  assert.equal((await call(A, "GET", `/v1/pair/${late}`)).status, 404);
});

test("met en cache la vérification SSO", async () => {
  now += 2 * 60_000;
  const before = ssoCalls;
  await call(A, "GET", "/v1/state");
  await call(A, "GET", "/v1/state");
  assert.equal(ssoCalls - before, 1);
});

test("suppression du compte", async () => {
  assert.equal((await call(A, "DELETE", "/v1/account")).status, 200);
  assert.deepEqual((await call(A, "GET", "/v1/state")).body, { keyset: null, seq: 0, bytes: 0 });
});
