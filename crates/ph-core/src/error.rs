use serde::Serialize;

pub type Result<T, E = CoreError> = std::result::Result<T, E>;

/// Erreurs du cœur. Volontairement pauvres en détails : aucune donnée
/// utilisateur ne doit fuiter dans un message d'erreur ou un journal.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("le coffre est verrouillé")]
    Locked,
    #[error("le coffre n'est pas encore créé")]
    NotInitialized,
    #[error("le coffre existe déjà")]
    AlreadyInitialized,
    #[error("phrase secrète incorrecte")]
    BadPassphrase,
    #[error("phrase secrète trop courte")]
    WeakPassphrase,
    #[error("champ invalide : {0}")]
    Invalid(&'static str),
    #[error("introuvable")]
    NotFound,
    #[error("fichier de clé illisible ou altéré")]
    CorruptKeyFile,
    #[error("version de données plus récente que l'application")]
    FutureSchema,
    #[error("erreur cryptographique")]
    Crypto,
    #[error("erreur de stockage : {0}")]
    Storage(#[from] rusqlite::Error),
    #[error("erreur d'entrée/sortie : {0}")]
    Io(#[from] std::io::Error),
}

impl CoreError {
    /// Code stable exposé à l'interface, qui le traduit. Ne jamais afficher
    /// le message brut d'une erreur de stockage à l'utilisateur.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Locked => "locked",
            Self::NotInitialized => "not_initialized",
            Self::AlreadyInitialized => "already_initialized",
            Self::BadPassphrase => "bad_passphrase",
            Self::WeakPassphrase => "weak_passphrase",
            Self::Invalid(_) => "invalid",
            Self::NotFound => "not_found",
            Self::CorruptKeyFile => "corrupt_key_file",
            Self::FutureSchema => "future_schema",
            Self::Crypto => "crypto",
            Self::Storage(_) => "storage",
            Self::Io(_) => "io",
        }
    }
}

/// Forme sérialisée envoyée au front : `{ code, field? }`.
impl Serialize for CoreError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("CoreError", 2)?;
        st.serialize_field("code", self.code())?;
        st.serialize_field(
            "field",
            &match self {
                Self::Invalid(f) => Some(*f),
                _ => None,
            },
        )?;
        st.end()
    }
}
