# Changelog

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
