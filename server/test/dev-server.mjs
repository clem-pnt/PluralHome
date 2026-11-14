// Serveur de dev SANS SSO : le jeton sert directement d'identifiant de compte.
// Uniquement pour les tests locaux (`cargo test -p ph-remote --test cloud_http -- --ignored`).
import { createServer } from "node:http";
import { createHandler } from "../app.mjs";
import { openStore } from "../store.mjs";

const port = Number(process.env.PORT ?? 3191);
const store = openStore(":memory:");
const verify = async (token) => `dev:${token}`;
createServer(createHandler({ store, verify })).listen(port, "127.0.0.1", () =>
  console.log(`dev sync sur http://127.0.0.1:${port}`),
);
