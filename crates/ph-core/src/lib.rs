//! Cœur de `PluralHome` : domaine, chiffrement et stockage local.
//!
//! Toutes les données vivent sur l'appareil, dans une base `SQLCipher` dont la clé
//! (DEK) n'existe en clair qu'en mémoire, le temps d'une session déverrouillée.
//! Sur disque, la DEK est enveloppée par une clé dérivée de la phrase secrète.

pub mod error;
pub mod front;
pub mod keyfile;
pub mod members;
pub mod messages;
mod migrations;
mod time;
pub mod vault;

pub use error::{CoreError, Result};
pub use vault::{Vault, VaultStatus};
