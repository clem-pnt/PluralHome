use time::OffsetDateTime;

use crate::{CoreError, Result};

/// Horodatage courant en millisecondes Unix, l'unité de toute la base.
pub fn now_ms() -> Result<i64> {
    i64::try_from(OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000)
        .map_err(|_| CoreError::Invalid("clock"))
}
