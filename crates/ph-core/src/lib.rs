//! Cœur de `PluralHome` : domaine, chiffrement et stockage local.
//!
//! Toutes les données vivent sur l'appareil, dans une base `SQLCipher` dont la clé
//! (DEK) n'existe en clair qu'en mémoire, le temps d'une session déverrouillée.
//! Sur disque, la DEK est enveloppée par une clé dérivée de la phrase secrète.

pub mod cloudkey;
pub mod error;
pub mod folders;
pub mod front;
pub mod groups;
pub mod journal;
pub mod keyfile;
mod keystore;
pub mod kv;
pub mod members;
pub mod messages;
mod migrations;
pub mod newcomer;
pub mod outbox;
pub mod polls;
pub mod replica;
mod reset;
pub mod roles;
pub mod safety;
pub mod system;
mod time;
pub mod vault;

pub use error::{CoreError, Result};
pub use rusqlite;
pub use vault::{Vault, VaultStatus};
