# Changelog

## 0.11.0 — Sous-dossiers, front détaillé, vote et accueil des nouveaux alters

- Dossiers : sous-dossiers (par exemple Âge > Enfants, Ados, Adultes), avec une mention « Ageslider » possible. Le tri automatique range aussi par genre. Barre de recherche dans l'écran Membres.
- Rôles : liste refaite, boutons Modifier et Supprimer en icônes.
- Au front : le jour est indiqué, bouton « Historique » pour tout l'historique du front, statut de chacun (au front, co-front, co-conscience). Le choix « Changer » a une barre de recherche et reste fluide avec des centaines d'alters. Le switch rapide montre les plus fréquents, ou des favoris à choisir.
- Messages : ceux reçus ou envoyés par les alters au front sont mis en avant. Plusieurs auteurs et plusieurs destinataires possibles. Un message peut être modifié. Le destinataire a un bouton « Marquer comme lu », et « Supprimer » ne s'affiche que pour lui. Seuls les messages non lus sont affichés par défaut, l'historique est à part.
- Nouvel onglet Vote : questions à choix entre alters, résultats en direct, clôture.
- Nouvel onglet Paramètres, où se trouvent maintenant les connexions (compte, synchro, PluralKit).
- Ancrage : le plan de crise est remplacé par un plan d'accueil pour les nouveaux alters (mot d'accueil, repères, personnes et lieux sûrs, premières choses à faire). Dans « Appeler à l'aide », un numéro personnalisable pour une personne de confiance.

## 0.10.4 — Tri automatique

- Tri automatique des membres d'après leur description : bouton « Trier automatiquement » dans « Gérer les dossiers ». Il propose des dossiers par âge (Enfants, Ados, Adultes) et par genre, et des rôles à ajouter (hôte, gatekeeper, protection, little…). L'analyse se fait sur l'appareil et rien n'est envoyé. Seules les mentions claires sont retenues (« Âge : 16 ans », « Genre : fille », « Rôle : gatekeeper », « c'est une protectrice »), et chaque proposition peut être décochée avant d'appliquer.
- Nouveaux rôles proposés : Co-hôte, Persécution, Fictif·ve.

## 0.10.3 — Profils et dossiers

- La liste des membres n'affiche plus que le rôle principal (le premier), sans aperçu de description.
- Un clic sur un membre ouvre son profil complet : tous ses rôles, ses dossiers et toute sa description. Le bouton « Modifier » ouvre la fiche d'édition.
- Plus de limite au nombre de rôles. Sur la fiche, la flèche d'un rôle en fait le rôle principal.
- Dossiers pour ranger les membres : bouton « Gérer les dossiers » dans l'écran Membres, filtre par dossier au-dessus de la liste. Un membre peut être dans plusieurs dossiers et reste toujours dans « Tous ». Les dossiers suivent la synchro entre appareils.

## 0.10.2 — Rôles et descriptions

- Chaque système a son propre catalogue de rôles : bouton « Gérer les rôles » dans l'écran Membres pour en créer, renommer ou supprimer. Renommer ou supprimer un rôle le change chez tous les membres qui le portent.
- Les rôles créés sont proposés sur la fiche de chaque membre, avec les rôles courants, et suivent la synchro entre appareils. Les rôles déjà utilisés rejoignent le catalogue à la mise à jour.
- PluralKit : nouvelle option pour synchroniser aussi les descriptions, dans les deux sens (désactivée par défaut, car une description peut être visible sur PluralKit). Au-delà de 1000 caractères, la description reste dans PluralHome.
- Une modification de profil relance aussitôt la synchro.

## 0.10.1 — Photos PluralKit

- Les photos de profil PluralKit sont importées pour les membres qui n'en ont pas ici : téléchargées une fois, réduites, puis rangées dans le coffre chiffré.
- Une photo choisie dans PluralHome n'est jamais remplacée, et une photo retirée ici ne revient pas (sauf si elle change sur PluralKit).
- Rien n'est envoyé à PluralKit : la synchro des photos ne va que dans un sens.

## 0.10.0 — Synchro entre appareils

- Photos de profil à la place des pastilles de couleur, partout dans l'application.
- Synchro entre appareils par le compte Aragonite : tout est chiffré sur l'appareil avant l'envoi, le serveur ne garde que des données illisibles.
- Une clé de synchronisation s'affiche une seule fois à l'activation : elle sert à ajouter un autre appareil, à noter et garder en lieu sûr.
- Les sauvegardes locales `.phbackup` disparaissent, remplacées par la synchro.
- Si la clé du coffre disparaît, l'écran d'erreur propose de repartir de zéro ; l'ancien coffre est mis de côté, pas effacé.

## 0.9.0 — Plus de phrase secrète

- L'application s'ouvre directement : la clé du coffre est rangée dans le trousseau de la session (GNOME Keyring, KWallet, Trousseau macOS, Gestionnaire d'identifiants Windows), ou à défaut dans un fichier protégé du dossier de l'application.
- Les coffres existants demandent la phrase secrète une dernière fois, puis passent au nouveau système.
- Plus de verrouillage automatique ni de bouton « Verrouiller ».
- Les sauvegardes sont protégées par un mot de passe choisi à chaque export (8 caractères minimum). Les anciennes sauvegardes se restaurent avec l'ancienne phrase secrète.
- Si la clé disparaît de l'appareil, un écran le dit clairement et propose de restaurer une sauvegarde.

## 0.8.0 — Journal partagé

- Nouvel écran « Journal » : ce qui s'est passé, écrit par qui était là, pour retrouver le fil d'une journée malgré l'amnésie.
- Une entrée peut avoir plusieurs auteurs (co-front) ; par défaut, les membres au front la signent. Aucun auteur coché : auteur inconnu.
- Titre facultatif, humeur facultative (de « Très dur » à « Très bien »), entrées classées par jour.
- Modification (la date d'écriture reste, la mention « modifié » apparaît) et suppression avec confirmation.
- Filtre par membre. Tout reste dans le coffre chiffré, rien n'est envoyé à PluralKit.

## 0.7.0 — Profils des membres

- Image de profil : recadrée au carré et réduite sur l'appareil, métadonnées (GPS, appareil photo) retirées au passage.
- Description libre et rôles (suggestions courantes ou libellés personnels), visibles sur la fiche de chaque membre.
- Ce profil détaillé reste sur l'appareil, dans le coffre chiffré : il n'est jamais envoyé à PluralKit.

## 0.6.0 — Ancrage et plan de crise

- Écran « Ancrage » : respiration au carré guidée, exercice 5-4-3-2-1, repères « ici et maintenant » (date, heure, phrases d'orientation).
- Accessible depuis l'écran de déverrouillage, sans la phrase secrète : rien n'y lit les données du coffre.
- Numéros d'aide gratuits (3114, 15, 112, 114), visibles même coffre verrouillé.
- Plan de crise personnel, chiffré dans le coffre : signes d'alerte, ce qui aide, raisons de tenir, personnes à joindre.

## 0.5.0 — Connexions

- Compte Aragonite facultatif : connexion par le navigateur (OAuth 2 + PKCE), affichage du compte Discord rattaché. Aucune donnée du coffre n'est envoyée.
- Synchro PluralKit dans les deux sens, tant que l'application est ouverte : membres (nom, pronoms, couleur) en fusion à trois voies, switchs envoyés aussitôt et reçus en moins d'une minute, sans écho. File d'envoi conservée hors ligne.
- Modification et archivage des membres ; noms et pronoms jusqu'à 100 caractères, comme sur PluralKit.

## 0.4.0 — Sauvegarde chiffrée

- Export du coffre en un seul fichier `.phbackup`, chiffré, qui s'ouvre avec la phrase secrète du moment.
- Restauration vérifiée de bout en bout (phrase secrète, intégrité de chaque page) avant de toucher au coffre actuel, qui est mis de côté et jamais effacé.
- Restauration possible dès le premier écran d'une nouvelle installation.

## 0.3.0 — Pont contre l'amnésie

- Messagerie interne : messages adressés à un membre ou à tout le système, épinglage, suppression avec confirmation.
- « Bon retour » : au retour au front, chaque membre voit qui est passé depuis son dernier passage, combien de temps, et les messages laissés pour lui.
- « J'ai lu » range le tout jusqu'au prochain passage.

## 0.2.0 — Suivi du front

- Écran « Au front » : qui est là maintenant et depuis quand.
- Switch rapide en un geste, co-front, et front « flou » quand on ne sait pas qui est là.
- Frise de la journée, une ligne par membre.
- Navigation entre « Au front » et « Membres ».

## 0.1.0 — Fondations

- Coffre local chiffré : base SQLCipher, clé protégée par phrase secrète (Argon2id + XChaCha20-Poly1305).
- Membres du système : création et liste.
- Interface en français, prête pour d'autres langues.
- Paquet .deb avec mises à jour automatiques signées.
