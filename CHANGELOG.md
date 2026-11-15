# Changelog

## Non publié

- Installation : on choisit d'abord si l'on est un système ou une seule personne (singlet), puis si l'on se connecte à un compte Aragonite ou si l'on continue en invité.
- Mode singlet : le front, les membres et les votes sont masqués, sans rien supprimer. Le profil se change dans Paramètres > Système.
- Connexion Aragonite : elle s'ouvre dans une fenêtre de l'application (session privée) au lieu du navigateur. Fermer la fenêtre annule la connexion.
- Compte Aragonite : sa photo de profil s'affiche dans Paramètres. Elle est gardée dans le coffre chiffré et mise à jour à chaque lancement.
- Se déconnecter d'Aragonite fait sortir de l'application : un écran propose de se reconnecter ou de continuer en invité. Les données restent sur l'appareil. C'est pareil si la session est fermée côté Aragonite.
- Front : on ne peut plus enregistrer un front sans personne « au front ». Il faut au moins un alter au statut « Au front », ou cocher « Flou ». Les switchs reçus de PluralKit ne sont pas concernés.
- Historique du front : on ne peut plus remonter avant le tout premier passage enregistré.
- Paramètres (et toutes les pages) : on ne peut plus faire défiler la fenêtre au-delà du contenu, ce qui décalait toute l'application. La barre latérale défile seule si la fenêtre est trop basse.
- Rattrapage : ajouter un alter au front ne fait plus croire aux alters déjà présents qu'ils viennent de revenir. La notification de retour ne liste que ce qui s'est passé pendant leur absence.
- Amis : plus de bouton « Synchroniser », la synchronisation est automatique (en arrière-plan et à l'ouverture de la page).
- Votes : date et/ou heure de fin facultative. Le vote se clôt tout seul à l'échéance (le jour seul : fin à 23 h 59 ; l'heure seule : aujourd'hui, ou demain si elle est passée). Rouvrir un vote retire sa date de fin.
- Votes : notification sur la page Front pour les votes en cours auxquels un membre présent n'a pas encore répondu, et pour les votes terminés dont le résultat n'a pas encore été vu. Une pastille sur « Vote » en donne le nombre. Les votes déjà clos avant la mise à jour comptent comme vus.
- Messages : on peut modifier toutes les discussions, y compris les discussions privées et celles créées avant la mise à jour (nom, photo, membres). La discussion devient un groupe et garde ses messages. Pour une discussion avec un ami, seuls vos messages passent dans le groupe ; les siens restent dans l'ancienne discussion de son côté.

## 0.12.1 — Correctif écran vide

- L'application restait blanche au démarrage depuis la 0.12.0 : la bibliothèque de QR codes plantait à cause d'une protection de sécurité de l'app. Elle est remplacée ; la création et la lecture des QR codes d'amis fonctionnent.

## 0.12.0 — Amis et messages entre systèmes

- Amis : ajout d'autres systèmes par lien ou QR code (caméra ou image), avec demandes à accepter, refuser ou bloquer, et un code à comparer de vive voix. Les invitations peuvent expirer (24 h, 7 jours ou jamais), servir une seule fois et être révoquées. Le compte Aragonite sert d'identité, mais son identifiant n'est jamais affiché. Tout est chiffré de bout en bout.
- Amis : réglages de confidentialité par défaut, et pour chaque ami. On choisit si l'ami voit la liste des membres, leurs photos, pronoms et descriptions, ou qui est au front, et s'il peut nous écrire. Certains membres peuvent être toujours masqués. Les nouvelles demandes peuvent être fermées.
- Messages : la nouvelle conversation et le nouveau groupe proposent une liste de systèmes, le nôtre et ceux des amis. Chacun a une liste déroulante de ses alters et un choix « Tout le système », pour les destinataires comme pour les auteurs.
- Messages : on peut écrire à un ami, ou à certains de ses alters, au nom de tout le système ou de certains alters. Un groupe peut être partagé avec un système ami. Supprimer un message envoyé le retire aussi chez l'ami ; un message reçu ne disparaît que chez soi.

## 0.11.2 — Groupes, épingles, réponses et recherche dans les messages

- Messages : groupes nommés, avec photos des membres empilées comme sur Discord ; clic droit sur un groupe pour changer son nom, sa photo et ses membres.
- Messages : un message épinglé s'affiche en haut de la conversation, cliquer dessus y ramène. Répondre à un message avec le bouton ou un double clic, la réponse cite le message d'origine.
- Messages : « Nouvelle conversation » ouvre une fenêtre pour choisir entre « Nouvelle discussion » et « Nouveau groupe », puis les membres. Recherche dans tous les messages.
- La conversation de tout le système porte le nom et la photo du système, repris de PluralKit ou choisis dans Paramètres > Système.
- Membres : rôles ajoutés avec un bouton « + » et une liste, comme sur Discord. Bouton « Archiver » qui range un membre en dormance dans une section à part, et « Supprimer » avec confirmation. Un membre supprimé venu de PluralKit n'y est pas supprimé, mais n'est plus réimporté.
- Personne de confiance : Téléphone et SMS fusionnés, l'e-mail ouvre la messagerie.
- Nouvel alter : plus aucune catégorie par défaut.
- Messages : l'onglet occupe toute la place à côté de la barre latérale, sans texte d'en-tête.
- Correction : un message écrit à plusieurs n'apparaît plus comme non lu, ni en notification, pour les alters qui l'ont écrit.

## 0.11.1 — Messagerie en conversations, notifications repliées

- Au front : bouton « Créer un alter » dans la fenêtre « Qui est au front ? ». « Flou » se coche comme un alter, sans statut. Le choix « Personne » est retiré.
- Notifications : au-delà de deux, elles sont repliées dans un seul bloc qu'on peut déplier, avec un bouton « Tout marquer comme lu ».
- Messages : nouvelle interface en conversations, à la manière des messages privés (liste à gauche, fil à droite, envoi avec Entrée). Seuls les messages qui concernent un alter présent (au front, co-front ou co-conscience) et ceux pour tout le système s'affichent. Bouton « Tout marquer comme lu ».
- Vote : un seul alter vote à la fois, choisir un autre alter remplace le précédent.
- Recherche : elle porte sur le prénom seul.
- Appeler à l'aide : le nom et l'utilité passent avant le numéro. La personne de confiance a plusieurs moyens de contact (téléphone, SMS, WhatsApp, Discord, Instagram, Snapchat…).
- Nouvel alter : catégories libres, à nommer et à ajouter autant que nécessaire.

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
