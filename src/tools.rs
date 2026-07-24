use base64::{prelude::BASE64_URL_SAFE_NO_PAD, Engine};
use rand::prelude::*;
use rand::rng;

pub fn token_urlsafe(nbytes: usize) -> String {
    let mut bytes = vec![0u8; nbytes];

    rng().fill_bytes(&mut bytes);

    BASE64_URL_SAFE_NO_PAD.encode(bytes)
}

pub fn token_auth(ndigits: usize) -> String {
    static CH0: u32 = '0' as u32;
    let mut rng = rand::rng();
    //SAFETY: We are generating digits from 0 to 9, which are valid Unicode scalar values.
    (0..ndigits).map(|_| unsafe { char::from_u32_unchecked(rng.random_range(0..10) + CH0) }).collect()
}

use axum::http::HeaderMap;

pub fn extract_header(headers: &HeaderMap, name: &str, default: &str) -> String {
    headers.get(name).and_then(|v| v.to_str().ok()).map(|s| s.to_string()).unwrap_or_else(|| default.to_string())
}

use chrono::Local;
use fern::colors::ColoredLevelConfig;
use fern::Dispatch;
use log::info;
use shadow_rs::shadow;

pub fn setup_logging(log_level: log::LevelFilter) -> Result<(), fern::InitError> {
    let colors = ColoredLevelConfig::new();

    Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{} [{}] [{}] {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                colors.color(record.level()),
                record.target(),
                message
            ))
        })
        .level(log::LevelFilter::Warn)
        .level_for(env!("CARGO_CRATE_NAME"), log_level)
        .chain(std::io::stdout())
        .apply()?;

    Ok(())
}

shadow!(build);

#[allow(unreachable_code)]
pub fn log_version_info(log_level: log::LevelFilter) -> () {
    info!(
        "Starting {} v{} ({}/{}{}) [{}]",
        build::PROJECT_NAME,
        build::PKG_VERSION,
        const {
            if !build::BRANCH.is_empty() {
                build::BRANCH
            } else {
                "unknown"
            }
        },
        const {
            if !build::SHORT_COMMIT.is_empty() {
                build::SHORT_COMMIT
            } else {
                "unknown"
            }
        },
        const {
            if !build::GIT_CLEAN {
                "/dirty"
            } else {
                ""
            }
        },
        build::BUILD_TIME
    );
    info!("  {}", build::RUST_VERSION);
    info!("  {}", build::RUST_CHANNEL);
    info!("  {}", build::CARGO_VERSION);
    info!("  configured log level: {}", log_level);
}
