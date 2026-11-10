//! Plan de crise du système : signes d'alerte, ce qui aide, raisons de tenir,
//! personnes à joindre. Stocké chiffré dans le coffre comme tout le reste ; les
//! numéros d'urgence publics, eux, sont dans l'interface et restent visibles coffre
//! verrouillé.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::{CoreError, Result, kv};

const KEY: &str = "safety.plan";
pub const MAX_ITEMS: usize = 20;
pub const MAX_TEXT: usize = 300;
pub const MAX_NAME: usize = 100;
pub const MAX_PHONE: usize = 32;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", default)]
pub struct SafetyPlan {
    /// Ce qui montre que ça ne va pas.
    pub warning_signs: Vec<String>,
    /// Ce qui aide à traverser le moment.
    pub coping: Vec<String>,
    /// Raisons de tenir.
    pub reasons: Vec<String>,
    /// Personnes à joindre.
    pub contacts: Vec<Contact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", default)]
pub struct Contact {
    pub name: String,
    pub phone: Option<String>,
}

fn clean_list(items: Vec<String>, field: &'static str) -> Result<Vec<String>> {
    let items: Vec<String> = items
        .into_iter()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect();
    if items.len() > MAX_ITEMS || items.iter().any(|s| s.chars().count() > MAX_TEXT) {
        return Err(CoreError::Invalid(field));
    }
    Ok(items)
}

fn clean_contact(c: Contact) -> Result<Option<Contact>> {
    let name = c.name.trim().to_owned();
    let phone = c
        .phone
        .map(|p| p.trim().to_owned())
        .filter(|p| !p.is_empty());
    if name.is_empty() && phone.is_none() {
        return Ok(None);
    }
    if name.is_empty() || name.chars().count() > MAX_NAME {
        return Err(CoreError::Invalid("contacts"));
    }
    if let Some(p) = &phone {
        let ok = p.len() <= MAX_PHONE
            && p.chars().any(|ch| ch.is_ascii_digit())
            && p.chars()
                .all(|ch| ch.is_ascii_digit() || " +.-()".contains(ch));
        if !ok {
            return Err(CoreError::Invalid("phone"));
        }
    }
    Ok(Some(Contact { name, phone }))
}

impl SafetyPlan {
    fn validated(self) -> Result<Self> {
        let contacts: Vec<Contact> = self
            .contacts
            .into_iter()
            .map(clean_contact)
            .filter_map(Result::transpose)
            .collect::<Result<_>>()?;
        if contacts.len() > MAX_ITEMS {
            return Err(CoreError::Invalid("contacts"));
        }
        Ok(Self {
            warning_signs: clean_list(self.warning_signs, "warningSigns")?,
            coping: clean_list(self.coping, "coping")?,
            reasons: clean_list(self.reasons, "reasons")?,
            contacts,
        })
    }
}

/// Plan enregistré, ou plan vide.
pub fn get(conn: &Connection) -> Result<SafetyPlan> {
    match kv::get(conn, KEY)? {
        // Un plan illisible ne doit jamais empêcher d'ouvrir l'écran d'ancrage.
        Some(json) => Ok(serde_json::from_str(&json).unwrap_or_else(|e| {
            tracing::error!("plan de crise illisible : {e}");
            SafetyPlan::default()
        })),
        None => Ok(SafetyPlan::default()),
    }
}

/// Remplace le plan (lignes vides ignorées) et renvoie la version enregistrée.
pub fn set(conn: &Connection, plan: SafetyPlan) -> Result<SafetyPlan> {
    let plan = plan.validated()?;
    let json = serde_json::to_string(&plan).map_err(|_| CoreError::Invalid("plan"))?;
    kv::set(conn, KEY, &json)?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vault;
    use crate::keyfile::KdfParams;

    fn vault() -> (tempfile::TempDir, Vault) {
        let dir = tempfile::tempdir().unwrap();
        let mut v = Vault::new(dir.path()).with_kdf(KdfParams::TEST);
        v.create("phrase de test").unwrap();
        (dir, v)
    }

    #[test]
    fn empty_by_default_then_round_trips_cleaned() {
        let (_d, v) = vault();
        let c = v.conn().unwrap();
        assert_eq!(get(c).unwrap(), SafetyPlan::default());
        let saved = set(
            c,
            SafetyPlan {
                warning_signs: vec!["  je m'isole ".into(), String::new()],
                coping: vec!["marcher".into()],
                reasons: vec![],
                contacts: vec![
                    Contact {
                        name: "Sam".into(),
                        phone: Some("06 12 34 56 78".into()),
                    },
                    Contact {
                        name: " ".into(),
                        phone: None,
                    },
                ],
            },
        )
        .unwrap();
        assert_eq!(saved.warning_signs, vec!["je m'isole"]);
        assert_eq!(saved.contacts.len(), 1);
        assert_eq!(get(c).unwrap(), saved);
    }

    #[test]
    fn rejects_bad_input() {
        let (_d, v) = vault();
        let c = v.conn().unwrap();
        let phone = |p: &str| SafetyPlan {
            contacts: vec![Contact {
                name: "Sam".into(),
                phone: Some(p.into()),
            }],
            ..SafetyPlan::default()
        };
        assert!(matches!(
            set(c, phone("appelle-moi")),
            Err(CoreError::Invalid("phone"))
        ));
        assert!(matches!(
            set(
                c,
                SafetyPlan {
                    coping: vec!["x".repeat(MAX_TEXT + 1)],
                    ..SafetyPlan::default()
                }
            ),
            Err(CoreError::Invalid("coping"))
        ));
        let nameless = SafetyPlan {
            contacts: vec![Contact {
                name: String::new(),
                phone: Some("15".into()),
            }],
            ..SafetyPlan::default()
        };
        assert!(matches!(
            set(c, nameless),
            Err(CoreError::Invalid("contacts"))
        ));
    }
}
