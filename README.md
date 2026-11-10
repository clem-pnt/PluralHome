<p align="center"><img src="design/logo/logo.svg" width="96" alt=""></p>

# PluralHome

Application de bureau pour les personnes vivant avec un trouble dissociatif (TDI, OSDD) :
un espace partagé où les membres du système se repèrent et se coordonnent.

**Tout reste sur l'appareil, chiffré.** Pas de compte, pas de serveur de données, pas de télémétrie.

## Architecture

```
crates/ph-core/          Domaine, chiffrement, stockage (Rust, sans dépendance à l'UI)
apps/desktop/src-tauri/  Coquille Tauri 2 : commandes IPC, journaux, mises à jour
apps/desktop/src/        Interface React 19 (React Compiler, React Aria, TanStack Query)
design/                  Logo, icônes, planche de direction artistique
scripts/publish.sh       Publication signée sur updates.aragonite.fr
```

### Sécurité des données

- Base **SQLCipher** (AES-256, pages authentifiées). La clé de données (DEK, 256 bits aléatoires)
  n'existe en clair qu'en mémoire, pendant une session déverrouillée.
- Sur disque, la DEK est enveloppée par une clé dérivée de la phrase secrète :
  **Argon2id** (64 Mio, 3 passes) puis **XChaCha20-Poly1305**. L'en-tête du fichier de clé
  est authentifié : un fichier altéré ou aux paramètres affaiblis est refusé.
- Changer de phrase secrète ne ré-chiffre que l'enveloppe, jamais la base.
- Verrouillage manuel ou après 10 minutes d'inactivité. Dossier en 0700, fichiers en 0600.
- Journaux locaux : événements et codes d'erreur uniquement, jamais de contenu saisi.
- CSP stricte, prototypes gelés, permissions Tauri au plus juste, instance unique.

### Contrat front / back

Les types TypeScript (`apps/desktop/src/ipc/bindings.ts`) sont générés depuis les signatures
Rust par tauri-specta : `pnpm --filter @pluralhome/desktop bindings`. Les erreurs traversent
la frontière sous forme de codes stables, traduits côté interface.

### Langues

Paraglide (inlang) : messages dans `apps/desktop/messages/<langue>.json`, compilés et typés.
Ajouter une langue = ajouter le fichier et la déclarer dans `project.inlang/settings.json`.

## Développement

```sh
pnpm install
pnpm dev            # application en mode développement
pnpm check          # types, lint, clippy (pedantic), tests
```

Prérequis Linux : Rust stable, Node 22+, pnpm, `libwebkit2gtk-4.1-dev`, `librsvg2-dev`.

## Publication

```sh
scripts/publish.sh
```

Construit le `.deb` signé, le dépose sur `updates.aragonite.fr/pluralhome/download/vX.Y.Z/`
et met à jour `latest.json`. Les applications installées le détectent au démarrage puis
toutes les 6 heures, l'installent en arrière-plan et proposent de redémarrer.

## Licence

GPL-3.0-or-later.
