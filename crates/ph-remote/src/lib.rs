//! Services distants. Rien ici ne touche au stockage sans passer par `ph-core` ;
//! les verrous sur la base ne sont jamais tenus pendant un appel réseau.

pub mod db;
pub mod error;
pub mod pluralkit;
pub mod sync;

pub use error::{RemoteError, Result};

/// Identifie l'application auprès des services (exigé par PluralKit).
pub const USER_AGENT: &str = concat!(
    "PluralHome/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/clem-pnt/PluralHome)"
);
