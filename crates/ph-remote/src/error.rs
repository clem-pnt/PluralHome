use ph_core::CoreError;

pub type Result<T, E = RemoteError> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum RemoteError {
    #[error("jeton refusé par le service")]
    Unauthorized,
    #[error("trop de requêtes, réessayer plus tard")]
    RateLimited,
    #[error("réponse {status} : {message}")]
    Http { status: u16, message: String },
    #[error("service injoignable : {0}")]
    Network(String),
    #[error("réponse illisible")]
    Decode,
    #[error("opération annulée")]
    Cancelled,
    #[error("délai dépassé")]
    Timeout,
    #[error("non configuré")]
    NotLinked,
    #[error(transparent)]
    Core(#[from] CoreError),
}

impl RemoteError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Unauthorized => "remote_unauthorized",
            Self::RateLimited => "remote_rate_limited",
            Self::Http { .. } => "remote_http",
            Self::Network(_) => "remote_network",
            Self::Decode => "remote_decode",
            Self::Cancelled => "remote_cancelled",
            Self::Timeout => "remote_timeout",
            Self::NotLinked => "remote_not_linked",
            Self::Core(e) => e.code(),
        }
    }
}

impl From<reqwest::Error> for RemoteError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_decode() {
            Self::Decode
        } else {
            Self::Network(e.without_url().to_string())
        }
    }
}
