// API HTTP de synchro PluralHome. L'authentification passe par le SSO Aragonite
// (jeton d'accès émis pour le client `pluralhome`) ; le contenu est chiffré de bout en bout.
import { createHash, randomUUID } from "node:crypto";
import { StoreError } from "./store.mjs";

const MAX_BODY = 8 * 1024 * 1024;
const MAX_VALUE = 700_000;
const MAX_PUSH = 1000;
const AUTH_TTL = 60_000;
const RATE_WINDOW = 60_000;
const RATE_MAX = 240;

const HEX64 = /^[0-9a-f]{64}$/;
const B64 = /^[A-Za-z0-9+/]*={0,2}$/;
const ID = /^[0-9A-Za-z-]{8,64}$/;

class HttpError extends Error {
  constructor(status, code) {
    super(code);
    this.status = status;
    this.code = code;
  }
}

const isB64 = (s, max) => typeof s === "string" && s.length > 0 && s.length <= max && B64.test(s);

/** Vérifie le jeton auprès du SSO ; renvoie l'identifiant stable du compte. */
export function ssoVerifier({ origin, clientId, fetchImpl = fetch, clock = Date.now }) {
  const cache = new Map();
  return async (token) => {
    const key = createHash("sha256").update(token).digest("hex");
    const hit = cache.get(key);
    if (hit && hit.exp > clock()) return hit.user;
    let res;
    try {
      res = await fetchImpl(`${origin}/oauth/userinfo`, {
        method: "POST",
        headers: {
          authorization: `Bearer ${token}`,
          "content-type": "application/x-www-form-urlencoded",
        },
        body: new URLSearchParams({ client_id: clientId }).toString(),
        signal: AbortSignal.timeout(10_000),
      });
    } catch {
      throw new HttpError(502, "SSO_UNREACHABLE");
    }
    if (res.status === 401 || res.status === 403) throw new HttpError(401, "UNAUTHORIZED");
    if (!res.ok) throw new HttpError(502, "SSO_ERROR");
    const body = await res.json().catch(() => null);
    const user = body?.user?.id;
    if (typeof user !== "string" || !user) throw new HttpError(502, "SSO_ERROR");
    if (cache.size > 5000) cache.clear();
    cache.set(key, { user, exp: clock() + AUTH_TTL });
    return user;
  };
}

function rateLimiter(clock) {
  const hits = new Map();
  return (key) => {
    const now = clock();
    let entry = hits.get(key);
    if (!entry || entry.reset <= now) {
      if (hits.size > 10_000) hits.clear();
      entry = { n: 0, reset: now + RATE_WINDOW };
      hits.set(key, entry);
    }
    if (++entry.n > RATE_MAX) throw new HttpError(429, "RATE_LIMITED");
  };
}

async function readJson(req) {
  const chunks = [];
  let size = 0;
  for await (const chunk of req) {
    size += chunk.length;
    if (size > MAX_BODY) throw new HttpError(413, "BODY_TOO_LARGE");
    chunks.push(chunk);
  }
  try {
    return JSON.parse(Buffer.concat(chunks).toString("utf8"));
  } catch {
    throw new HttpError(400, "BAD_JSON");
  }
}

function send(res, status, body) {
  const data = body === undefined ? "" : JSON.stringify(body);
  res.writeHead(status, {
    "content-type": "application/json; charset=utf-8",
    "cache-control": "no-store",
    "x-content-type-options": "nosniff",
    "content-length": Buffer.byteLength(data),
  });
  res.end(data);
}

export function createHandler({ store, verify, clock = Date.now }) {
  const limit = rateLimiter(clock);

  const routes = {
    "GET /v1/state": (user) => store.state(user),
    "PUT /v1/keyset": async (user, req) => {
      const body = await readJson(req);
      if (!ID.test(body?.id ?? "") || !isB64(body?.recovery, 1024)) throw new HttpError(400, "BAD_KEYSET");
      store.createKeyset(user, { id: body.id, recovery: body.recovery });
      return { ok: true };
    },
    "GET /v1/changes": (user, _req, url) => {
      const since = Number(url.searchParams.get("since") ?? 0);
      const max = Number(url.searchParams.get("limit") ?? 500);
      if (!Number.isSafeInteger(since) || since < 0) throw new HttpError(400, "BAD_SINCE");
      if (!Number.isSafeInteger(max) || max < 1 || max > MAX_PUSH) throw new HttpError(400, "BAD_LIMIT");
      const { items, more } = store.changes(user, since, max);
      const seq = items.length ? items[items.length - 1].seq : since;
      return { items: items.map(({ k, v, seq: s }) => ({ k, v, s })), seq, more };
    },
    "POST /v1/changes": async (user, req) => {
      const body = await readJson(req);
      const items = body?.items;
      if (!Array.isArray(items) || items.length === 0 || items.length > MAX_PUSH) throw new HttpError(400, "BAD_ITEMS");
      for (const item of items) {
        if (!HEX64.test(item?.k ?? "") || !isB64(item?.v, MAX_VALUE)) throw new HttpError(400, "BAD_ITEM");
      }
      return { seq: store.push(user, items) };
    },
    "DELETE /v1/account": (user) => {
      store.deleteAccount(user);
      return { ok: true };
    },
    "POST /v1/pair": async (user, req) => {
      const body = await readJson(req);
      if (!HEX64.test(body?.pub ?? "")) throw new HttpError(400, "BAD_PUB");
      const id = randomUUID();
      store.pairCreate(user, id, body.pub);
      return { id };
    },
    "GET /v1/pair": (user) => ({ requests: store.pairList(user) }),
  };

  const pairRoutes = {
    PUT: async (user, req, id) => {
      const body = await readJson(req);
      if (!isB64(body?.sealed, 1024)) throw new HttpError(400, "BAD_SEALED");
      store.pairSeal(user, id, body.sealed);
      return { ok: true };
    },
    GET: (user, _req, id) => ({ sealed: store.pairTake(user, id) }),
    DELETE: (user, _req, id) => {
      store.pairCancel(user, id);
      return { ok: true };
    },
  };

  return async (req, res) => {
    try {
      const url = new URL(req.url ?? "/", "http://localhost");
      if (req.method === "GET" && url.pathname === "/health") return send(res, 200, { ok: true });

      let handler = routes[`${req.method} ${url.pathname}`];
      let pairId;
      const pair = /^\/v1\/pair\/([0-9a-f-]{36})$/.exec(url.pathname);
      if (!handler && pair && pairRoutes[req.method]) {
        handler = pairRoutes[req.method];
        pairId = pair[1];
      }
      if (!handler) throw new HttpError(404, "NOT_FOUND");

      const auth = /^Bearer ([\x21-\x7e]{16,4096})$/.exec(req.headers.authorization ?? "");
      if (!auth) throw new HttpError(401, "UNAUTHORIZED");
      const user = await verify(auth[1]);
      limit(user);
      send(res, 200, await handler(user, req, pairId ?? url));
    } catch (error) {
      if (error instanceof HttpError || error instanceof StoreError) {
        return send(res, error.status, { error: error.code });
      }
      console.error("pluralhome-sync:", error);
      send(res, 500, { error: "INTERNAL" });
    }
  };
}
