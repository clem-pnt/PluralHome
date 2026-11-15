//! Plan d'accueil des nouveaux alters et personne de confiance à joindre.
//! Les deux vivent chiffrés dans le coffre (`kv`) et se synchronisent comme le
//! plan de crise qu'ils remplacent dans l'interface.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::safety::{self, MAX_ITEMS, MAX_NAME, MAX_PHONE, clean_list};
use crate::{CoreError, Result, kv};

const PLAN_KEY: &str = "newcomer.plan";
const TRUSTED_KEY: &str = "trusted.contact";
pub const MAX_WELCOME: usize = 2000;
pub const MAX_TITLE: usize = 100;
pub const MAX_CHANNELS: usize = 10;
pub const MAX_HANDLE: usize = 100;
/// Moyens de joindre la personne de confiance.
pub const CHANNEL_KINDS: &[&str] = &[
    "phone",
    "sms",
    "discord",
    "instagram",
    "snapchat",
    "whatsapp",
    "signal",
    "telegram",
    "messenger",
    "email",
    "other",
];

/// Une catégorie du plan, nommée par le système.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", default)]
pub struct PlanSection {
    pub title: String,
    pub items: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", default)]
pub struct NewcomerPlan {
    /// Mot d'accueil, lu en premier.
    pub welcome: String,
    /// Catégories libres, dans l'ordre d'affichage.
    pub sections: Vec<PlanSection>,
}

/// Forme stockée : accepte aussi les trois listes fixes d'avant la 0.12.
#[derive(Deserialize, Default)]
#[serde(default)]
struct StoredPlan {
    welcome: String,
    sections: Option<Vec<PlanSection>>,
    basics: Vec<String>,
    safe: Vec<String>,
    steps: Vec<String>,
}

impl From<StoredPlan> for NewcomerPlan {
    fn from(s: StoredPlan) -> Self {
        let sections = s.sections.unwrap_or_else(|| {
            [
                ("Repères", s.basics),
                ("Personnes et lieux sûrs", s.safe),
                ("Premières choses à faire", s.steps),
            ]
            .into_iter()
            .filter(|(_, items)| !items.is_empty())
            .map(|(title, items)| PlanSection {
                title: title.into(),
                items,
            })
            .collect()
        });
        Self {
            welcome: s.welcome,
            sections,
        }
    }
}

impl NewcomerPlan {
    fn validated(self) -> Result<Self> {
        let welcome = self.welcome.trim().to_owned();
        if welcome.chars().count() > MAX_WELCOME {
            return Err(CoreError::Invalid("welcome"));
        }
        let mut sections = Vec::new();
        for s in self.sections {
            let title = s.title.trim().to_owned();
            let items = clean_list(s.items, "basics")?;
            if title.is_empty() && items.is_empty() {
                continue;
            }
            if title.is_empty() || title.chars().count() > MAX_TITLE {
                return Err(CoreError::Invalid("section_title"));
            }
            sections.push(PlanSection { title, items });
        }
        if sections.len() > MAX_ITEMS {
            return Err(CoreError::Invalid("sections"));
        }
        Ok(Self { welcome, sections })
    }
}

/// Plan enregistré, ou plan vide.
pub fn get_plan(conn: &Connection) -> Result<NewcomerPlan> {
    Ok(kv::get(conn, PLAN_KEY)?
        .and_then(|json| {
            serde_json::from_str::<StoredPlan>(&json)
                .inspect_err(|e| tracing::error!("plan nouveaux alters illisible : {e}"))
                .ok()
        })
        .map(NewcomerPlan::from)
        .unwrap_or_default())
}

/// Remplace le plan (lignes et catégories vides ignorées) et renvoie la version enregistrée.
pub fn set_plan(conn: &Connection, plan: NewcomerPlan) -> Result<NewcomerPlan> {
    let plan = plan.validated()?;
    let json = serde_json::to_string(&plan).map_err(|_| CoreError::Invalid("plan"))?;
    kv::set(conn, PLAN_KEY, &json)?;
    Ok(plan)
}

/// Un moyen de joindre la personne : `kind` parmi [`CHANNEL_KINDS`], `value` le
/// numéro, le pseudo ou l'adresse.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", default)]
pub struct ContactChannel {
    pub kind: String,
    pub value: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", default)]
pub struct TrustedContact {
    pub name: String,
    pub channels: Vec<ContactChannel>,
}

/// Forme stockée : accepte aussi l'ancien `{ name, phone }`.
#[derive(Deserialize, Default)]
#[serde(default)]
struct StoredTrusted {
    name: String,
    phone: Option<String>,
    channels: Option<Vec<ContactChannel>>,
}

fn phone_channel(phone: String) -> ContactChannel {
    ContactChannel {
        kind: "phone".into(),
        value: phone,
    }
}

impl From<StoredTrusted> for TrustedContact {
    fn from(s: StoredTrusted) -> Self {
        let channels = s
            .channels
            .unwrap_or_else(|| s.phone.into_iter().map(phone_channel).collect());
        Self {
            name: s.name,
            channels: merge_sms(channels),
        }
    }
}

/// SMS et appel passent par le même numéro : un ancien moyen « sms » devient
/// « phone », et un numéro déjà présent n'est pas répété.
fn merge_sms(channels: Vec<ContactChannel>) -> Vec<ContactChannel> {
    let mut out: Vec<ContactChannel> = Vec::with_capacity(channels.len());
    for mut c in channels {
        if c.kind == "sms" {
            c.kind = "phone".into();
        }
        if !out.iter().any(|o| o.kind == c.kind && o.value == c.value) {
            out.push(c);
        }
    }
    out
}

fn valid_phone(p: &str) -> bool {
    p.len() <= MAX_PHONE
        && p.chars().any(|ch| ch.is_ascii_digit())
        && p.chars()
            .all(|ch| ch.is_ascii_digit() || " +.-()".contains(ch))
}

impl TrustedContact {
    /// `None` si tout est vide.
    fn validated(self) -> Result<Option<Self>> {
        let name = self.name.trim().to_owned();
        let mut channels = Vec::new();
        for c in self.channels {
            let value = c.value.trim().to_owned();
            if value.is_empty() {
                continue;
            }
            if !CHANNEL_KINDS.contains(&c.kind.as_str()) {
                return Err(CoreError::Invalid("channel"));
            }
            let ok = match c.kind.as_str() {
                "phone" | "sms" | "whatsapp" => valid_phone(&value),
                _ => value.chars().count() <= MAX_HANDLE,
            };
            if !ok {
                return Err(CoreError::Invalid(
                    if c.kind == "phone" || c.kind == "sms" {
                        "phone"
                    } else {
                        "channel"
                    },
                ));
            }
            channels.push(ContactChannel {
                kind: c.kind,
                value,
            });
        }
        let channels = merge_sms(channels);
        if name.is_empty() && channels.is_empty() {
            return Ok(None);
        }
        if name.is_empty() || name.chars().count() > MAX_NAME {
            return Err(CoreError::Invalid("contacts"));
        }
        if channels.is_empty() {
            return Err(CoreError::Invalid("channels"));
        }
        if channels.len() > MAX_CHANNELS {
            return Err(CoreError::Invalid("channels"));
        }
        Ok(Some(Self { name, channels }))
    }
}

/// Personne de confiance. Tant qu'aucune n'est choisie, reprend la première personne
/// avec un numéro de l'ancien plan de crise, pour ne rien perdre.
pub fn get_trusted(conn: &Connection) -> Result<Option<TrustedContact>> {
    if let Some(json) = kv::get(conn, TRUSTED_KEY)? {
        return Ok(serde_json::from_str::<Option<StoredTrusted>>(&json)
            .unwrap_or_default()
            .map(TrustedContact::from));
    }
    Ok(safety::get(conn)?.contacts.into_iter().find_map(|c| {
        c.phone.map(|p| TrustedContact {
            name: c.name,
            channels: vec![phone_channel(p)],
        })
    }))
}

/// Enregistre la personne de confiance ; `None` (ou tout vide) l'efface.
/// Au moins un moyen de la joindre est obligatoire.
pub fn set_trusted(
    conn: &Connection,
    contact: Option<TrustedContact>,
) -> Result<Option<TrustedContact>> {
    let contact = contact
        .map(TrustedContact::validated)
        .transpose()?
        .flatten();
    let json = serde_json::to_string(&contact).map_err(|_| CoreError::Invalid("contact"))?;
    kv::set(conn, TRUSTED_KEY, &json)?;
    Ok(contact)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vault;
    use crate::keyfile::KdfParams;
    use crate::safety::{Contact, SafetyPlan};

    fn vault() -> (tempfile::TempDir, Vault) {
        let dir = tempfile::tempdir().unwrap();
        let mut v = Vault::new(dir.path()).with_kdf(KdfParams::TEST);
        v.create().unwrap();
        (dir, v)
    }

    fn section(title: &str, items: &[&str]) -> PlanSection {
        PlanSection {
            title: title.into(),
            items: items.iter().map(|s| (*s).into()).collect(),
        }
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
                sections: vec![
                    section(" Où on est ", &["On est en 2026", " "]),
                    section("", &[]),
                    section("Animaux", &["Le chat s'appelle Miso"]),
                ],
            },
        )
        .unwrap();
        assert_eq!(saved.welcome, "Bienvenue, tu es en sécurité.");
        assert_eq!(saved.sections.len(), 2);
        assert_eq!(saved.sections[0], section("Où on est", &["On est en 2026"]));
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
        assert!(matches!(
            set_plan(
                c,
                NewcomerPlan {
                    sections: vec![section("", &["sans titre"])],
                    ..NewcomerPlan::default()
                }
            ),
            Err(CoreError::Invalid("section_title"))
        ));
    }

    #[test]
    fn old_plan_becomes_sections() {
        let (_d, v) = vault();
        let c = v.conn().unwrap();
        kv::set(
            c,
            PLAN_KEY,
            r#"{"welcome":"Salut","basics":["2026"],"safe":[],"steps":["Boire de l'eau"]}"#,
        )
        .unwrap();
        let plan = get_plan(c).unwrap();
        assert_eq!(plan.welcome, "Salut");
        assert_eq!(
            plan.sections,
            vec![
                section("Repères", &["2026"]),
                section("Premières choses à faire", &["Boire de l'eau"]),
            ]
        );
    }

    fn channel(kind: &str, value: &str) -> ContactChannel {
        ContactChannel {
            kind: kind.into(),
            value: value.into(),
        }
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
        let sam = get_trusted(c).unwrap().unwrap();
        assert_eq!(sam.name, "Sam");
        assert_eq!(sam.channels, vec![channel("phone", "06 12 34 56 78")]);

        let lou = TrustedContact {
            name: " Lou ".into(),
            channels: vec![
                channel("discord", " lou.42 "),
                channel("phone", "07 00 00 00 00"),
                channel("instagram", ""),
            ],
        };
        let saved = set_trusted(c, Some(lou)).unwrap().unwrap();
        assert_eq!(saved.name, "Lou");
        assert_eq!(
            saved.channels,
            vec![
                channel("discord", "lou.42"),
                channel("phone", "07 00 00 00 00")
            ]
        );
        assert_eq!(get_trusted(c).unwrap(), Some(saved));

        // Un ancien « SMS » se fond dans le téléphone.
        let merged = set_trusted(
            c,
            Some(TrustedContact {
                name: "Lou".into(),
                channels: vec![
                    channel("phone", "07 00 00 00 00"),
                    channel("sms", "07 00 00 00 00"),
                    channel("sms", "06 00 00 00 00"),
                ],
            }),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            merged.channels,
            vec![
                channel("phone", "07 00 00 00 00"),
                channel("phone", "06 00 00 00 00")
            ]
        );

        let bad = |channels| TrustedContact {
            name: "Lou".into(),
            channels,
        };
        assert!(matches!(
            set_trusted(c, Some(bad(vec![]))),
            Err(CoreError::Invalid("channels"))
        ));
        assert!(matches!(
            set_trusted(c, Some(bad(vec![channel("phone", "pas un numéro")]))),
            Err(CoreError::Invalid("phone"))
        ));
        assert!(matches!(
            set_trusted(c, Some(bad(vec![channel("fax", "123")]))),
            Err(CoreError::Invalid("channel"))
        ));
        // Effacée volontairement : plus de repli sur l'ancien plan.
        assert_eq!(set_trusted(c, None).unwrap(), None);
        assert_eq!(get_trusted(c).unwrap(), None);
    }

    #[test]
    fn old_trusted_keeps_its_phone() {
        let (_d, v) = vault();
        let c = v.conn().unwrap();
        kv::set(c, TRUSTED_KEY, r#"{"name":"Sam","phone":"06 12 34 56 78"}"#).unwrap();
        let sam = get_trusted(c).unwrap().unwrap();
        assert_eq!(sam.channels, vec![channel("phone", "06 12 34 56 78")]);
    }
}
