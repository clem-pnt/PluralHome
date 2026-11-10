# Synchro PluralKit et compte Aragonite

Les deux sont facultatifs. Sans eux, PluralHome fonctionne entièrement en local.

## Compte Aragonite

- Client OAuth 2 public `pluralhome` sur `sso.aragonite.fr` : code d'autorisation + PKCE S256, redirection
  de bouclage `http://127.0.0.1:<port>/callback` (RFC 8252), port choisi par le système à chaque connexion.
- L'application ouvre le navigateur, écoute le retour 5 minutes au plus (annulable), vérifie le `state`
  en temps constant et ignore toute autre requête locale.
- Seul le jeton de renouvellement est conservé, dans le coffre chiffré (`kv aragonite.refresh_token`).
  Le SSO le fait tourner à chaque usage et ferme la session si un ancien est rejoué : le nouveau est
  enregistré avant toute autre opération. Un renouvellement refusé fait oublier la session localement.
- Données lues : nom affiché et identité Discord (principale en priorité), via `POST /oauth/userinfo`.
- Déconnexion : `POST /oauth/revoke`, puis oubli local même si le SSO est injoignable.
- Le compte ne reçoit aucune donnée du coffre.

Code : `crates/ph-remote/src/aragonite.rs`.

## PluralKit

Client de l'API v2 côté application, jeton dans le coffre chiffré (`kv pluralkit.token`). Pas de serveur
intermédiaire ni de webhook : un serveur qui reçoit les événements PluralKit verrait les données en clair.

### Ce qui est partagé

Noms, pronoms, couleurs des membres et switchs. Rien d'autre (messages, notes, historique détaillé) ne
quitte l'appareil. L'écran de liaison rappelle que PluralKit stocke ces données sans chiffrement de bout en bout.

### Membres

- Fusion à trois voies, champ par champ, contre le dernier état synchronisé (`pk_member.snapshot`).
  Changé d'un seul côté : propagé. Changé des deux côtés : la version locale l'emporte.
- À la liaison, un membre présent des deux côtés sous le même nom (sans casse) prend les valeurs de PluralKit.
- Membre créé ici : créé sur PluralKit. Membre créé sur PluralKit : créé ici.
- Supprimé sur PluralKit : délié, jamais effacé ici. Archivé ici : plus touché par la synchro.
- Couleur absente sur PluralKit : pas d'avis, la couleur locale est gardée. Champs PluralKit non gérés
  (avatar, description…) : jamais modifiés.

### Switchs

- Un switch local est mis en file d'envoi (`outbox`) dans la même transaction que le changement ; la file
  part dans l'ordre, survit à l'arrêt et au mode hors ligne. Une erreur 400 abandonne l'entrée, une autre
  erreur arrête l'envoi en conservant l'ordre.
- Les 20 derniers switchs PluralKit sont lus ; chacun est mémorisé par son identifiant (`pk_switch`),
  donc ni écho ni boucle. Un switch distant n'est appliqué que s'il est plus récent que la liaison et que
  le dernier changement local.
- Le front « flou » part comme « personne au front » (liste vide).

### Rythme

Boucle d'arrière-plan (`apps/desktop/src-tauri/src/remote.rs`), uniquement coffre ouvert et PluralKit lié :
switchs toutes les 30 s, membres toutes les 5 min. Un switch local réveille la boucle aussitôt ; une
modification de membre ou le déverrouillage déclenchent une synchro complète. Une seule synchro à la fois.
Le verrou du coffre n'est jamais tenu pendant un appel réseau. Limites PluralKit respectées : écritures
espacées de 400 ms, `retry_after` suivi sur 429.

### État visible

`Connexions > PluralKit` : système lié, dernière synchro, switchs en attente, code de la dernière erreur
(effacé au premier succès), bouton « Synchroniser ». Délier oublie le jeton, les correspondances et la
file ; rien n'est supprimé, ni ici ni sur PluralKit.

Code : `crates/ph-remote/src/{pluralkit,sync}.rs`, tests : `crates/ph-remote/tests/sync.rs`.
