//! Plan d'accueil des nouveaux alters et personne de confiance à appeler.
//! Les deux vivent chiffrés dans le coffre (`kv`) et se synchronisent comme le
//! plan de crise qu'ils remplacent dans l'interface.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::safety::{self, Contact, clean_contact, clean_list};
use crate::{CoreError, Result, kv};

const PLAN_KEY: &str = "newcomer.plan";
const TRUSTED_KEY: &str = "trusted.contact";
pub const MAX_WELCOME: usize = 2000;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", default)]
pub struct NewcomerPlan {
    /// Mot d'accueil, lu en premier.
    pub welcome: String,
    /// Repères : où on est, quelle année, qui on est.
    pub basics: Vec<String>,
    /// Personnes et lieux sûrs.
    pub safe: Vec<String>,
    /// Premières choses à faire.
    pub steps: Vec<String>,
}

impl NewcomerPlan {
    fn validated(self) -> Result<Self> {
        let welcome = self.welcome.trim().to_owned();
        if welcome.chars().count() > MAX_WELCOME {
            return Err(CoreError::Invalid("welcome"));
        }
        Ok(Self {
            welcome,
            basics: clean_list(self.basics, "basics")?,
            safe: clean_list(self.safe, "safe")?,
            steps: clean_list(self.steps, "steps")?,
        })
    }
}

/// Plan enregistré, ou plan vide.
pub fn get_plan(conn: &Connection) -> Result<NewcomerPlan> {
    Ok(kv::get(conn, PLAN_KEY)?
        .and_then(|json| {
            serde_json::from_str(&json)
                .inspect_err(|e| tracing::error!("plan nouveaux alters illisible : {e}"))
                .ok()
        })
        .unwrap_or_default())
}

/// Remplace le plan (lignes vides ignorées) et renvoie la version enregistrée.
pub fn set_plan(conn: &Connection, plan: NewcomerPlan) -> Result<NewcomerPlan> {
    let plan = plan.validated()?;
    let json = serde_json::to_string(&plan).map_err(|_| CoreError::Invalid("plan"))?;
    kv::set(conn, PLAN_KEY, &json)?;
    Ok(plan)
}

/// Personne de confiance. Tant qu'aucune n'est choisie, reprend la première personne
/// avec un numéro de l'ancien plan de crise, pour ne rien perdre.
pub fn get_trusted(conn: &Connection) -> Result<Option<Contact>> {
    if let Some(json) = kv::get(conn, TRUSTED_KEY)? {
        return Ok(serde_json::from_str::<Option<Contact>>(&json).unwrap_or_default());
    }
    Ok(safety::get(conn)?
        .contacts
        .into_iter()
        .find(|c| c.phone.is_some()))
}

/// Enregistre la personne de confiance ; `None` (ou nom et numéro vides) l'efface.
/// Un numéro est obligatoire : c'est fait pour appeler.
pub fn set_trusted(conn: &Connection, contact: Option<Contact>) -> Result<Option<Contact>> {
    let contact = contact.map(clean_contact).transpose()?.flatten();
    if contact.as_ref().is_some_and(|c| c.phone.is_none()) {
        return Err(CoreError::Invalid("phone"));
    }
    let json = serde_json::to_string(&contact).map_err(|_| CoreError::Invalid("contact"))?;
    kv::set(conn, TRUSTED_KEY, &json)?;
    Ok(contact)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vault;
    use crate::keyfile::KdfParams;
    use crate::safety::SafetyPlan;

    fn vault() -> (tempfile::TempDir, Vault) {
        let dir = tempfile::tempdir().unwrap();
        let mut v = Vault::new(dir.path()).with_kdf(KdfParams::TEST);
        v.create().unwrap();
        (dir, v)
    }

    #[test]
    fn plan_round_trips_cleaned() {
        let (_d, v) = vault();
        let c = v.conn().unwrap();
        assert_eq!(get_plan(c).unwrap(), NewcomerPlan::default());
        let saved = set_plan(
            c,
            NewcomerPlan {
                welcome: "  Bienvenue, tu es en sécurité. ".into(),
                basics: vec!["On est en 2026".into(), " ".into()],
                safe: vec![],
                steps: vec!["Boire un verre d'eau".into()],
            },
        )
        .unwrap();
        assert_eq!(saved.welcome, "Bienvenue, tu es en sécurité.");
        assert_eq!(saved.basics.len(), 1);
        assert_eq!(get_plan(c).unwrap(), saved);
        assert!(matches!(
            set_plan(
                c,
                NewcomerPlan {
                    welcome: "x".repeat(MAX_WELCOME + 1),
                    ..NewcomerPlan::default()
                }
            ),
            Err(CoreError::Invalid("welcome"))
        ));
    }

    #[test]
    fn trusted_falls_back_then_overrides() {
        let (_d, v) = vault();
        let c = v.conn().unwrap();
        assert_eq!(get_trusted(c).unwrap(), None);
        safety::set(
            c,
            SafetyPlan {
                contacts: vec![
                    Contact {
                        name: "Sans numéro".into(),
                        phone: None,
                    },
                    Contact {
                        name: "Sam".into(),
                        phone: Some("06 12 34 56 78".into()),
                    },
                ],
                ..SafetyPlan::default()
            },
        )
        .unwrap();
        assert_eq!(get_trusted(c).unwrap().unwrap().name, "Sam");
        let lou = Contact {
            name: " Lou ".into(),
            phone: Some("07 00 00 00 00".into()),
        };
        assert_eq!(set_trusted(c, Some(lou)).unwrap().unwrap().name, "Lou");
        assert_eq!(get_trusted(c).unwrap().unwrap().name, "Lou");
        let no_phone = Contact {
            name: "Lou".into(),
            phone: None,
        };
        assert!(matches!(
            set_trusted(c, Some(no_phone)),
            Err(CoreError::Invalid("phone"))
        ));
        // Effacée volontairement : plus de repli sur l'ancien plan.
        assert_eq!(set_trusted(c, None).unwrap(), None);
        assert_eq!(get_trusted(c).unwrap(), None);
    }
}
