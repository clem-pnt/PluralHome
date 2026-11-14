// Point d'entrée du serveur de synchro (écoute en local, derrière nginx).
import { createServer } from "node:http";
import { createHandler, ssoVerifier } from "./app.mjs";
import { openStore } from "./store.mjs";

const port = Number(process.env.PORT ?? 3190);
const host = process.env.HOST ?? "127.0.0.1";
const store = openStore(process.env.DATABASE_PATH ?? "./data/sync.db");
const verify = ssoVerifier({
  origin: process.env.SSO_ORIGIN ?? "http://127.0.0.1:3100",
  clientId: process.env.SSO_CLIENT_ID ?? "pluralhome",
});

const server = createServer(createHandler({ store, verify }));
server.requestTimeout = 60_000;
server.headersTimeout = 15_000;
server.listen(port, host, () => console.log(`pluralhome-sync sur http://${host}:${port}`));

for (const signal of ["SIGINT", "SIGTERM"]) {
  process.on(signal, () => server.close(() => {
    store.close();
    process.exit(0);
  }));
}
