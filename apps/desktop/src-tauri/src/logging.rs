//! Journaux locaux, rotation quotidienne, 14 fichiers maximum.
//! Règle : on journalise des événements et des codes, jamais de contenu saisi.

use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

pub fn init(dir: &Path) -> Result<WorkerGuard, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(dir)?;
    let file = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("pluralhome")
        .filename_suffix("log")
        .max_log_files(14)
        .build(dir)?;
    let (writer, guard) = tracing_appender::non_blocking(file);
    let filter = EnvFilter::try_from_env("PLURALHOME_LOG")
        .unwrap_or_else(|_| EnvFilter::new("info,tao=warn,wry=warn"));
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(writer).with_ansi(false))
        .with(cfg!(debug_assertions).then(|| fmt::layer().with_writer(std::io::stderr)))
        .try_init()?;
    std::panic::set_hook(Box::new(|info| {
        tracing::error!("panic : {info}");
    }));
    Ok(guard)
}
