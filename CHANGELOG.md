# Changelog

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
