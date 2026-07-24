mod auth_service;
mod config;
pub mod geoip_service;
pub mod store;
mod telegram_service;
pub mod templates;
pub(crate) mod tools;

use crate::auth_service::AuthService;
use crate::geoip_service::Geoip;
use crate::store::{Store, StoreConfig};
use crate::templates::Templates;
use crate::tools::{log_version_info, setup_logging};
use clap::Parser;
use config::AppConfig;
use log::{error, info, warn};
use std::env;
use std::error::Error;
use std::process::ExitCode;
use std::str::FromStr;
use std::sync::Arc;
use telegram_service::TelegramService;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> ExitCode {
    let level = log::LevelFilter::from_str(env::var("LOGLEVEL").unwrap_or_else(|_| "info".to_string()).as_ref())
        .unwrap_or_else(|_| log::LevelFilter::Info);

    setup_logging(level).expect("Failed to initialize logger");

    log_version_info(level);

    let config = AppConfig::parse();

    let templates = Templates::new();

    let store_config = StoreConfig { request_expiration: config.config_request_expiration.into() };

    let store = match Store::new(&config.valkey_server, store_config).await {
        Ok(store) => store,
        Err(e) => {
            error!("Valkey setup error for {}: {:?}", config.valkey_server, e);
            return ExitCode::FAILURE;
        }
    };

    let geoip = Geoip::new(&config.geoip_city, &config.geoip_asn)
        .inspect_err(|e| warn!("GeoIP databases not found or failed to load (city={}, asn={}): {}", config.geoip_city.display(), config.geoip_asn.display(), e))
        .map(Arc::new)
        .ok();

    let telegram_service = match async {
        let telegram_service =
            TelegramService::new(store.clone(), templates.clone(), &config.telegram_bot_token, None).await?;
        telegram_service.setup_webhook(&config.telegram_host_url).await?;
    info!("Telegram webhook for @{} set to {}", telegram_service.username(), config.telegram_host_url);
        Ok::<TelegramService, Box<dyn Error>>(telegram_service)
    }
        .await
    {
        Ok(telegram_service) => telegram_service,
        Err(e) => {
            error!("Telegram setup error: {:?}", e);
            return ExitCode::FAILURE;
        }
    };

    let shutdown_token = CancellationToken::new();

    tokio::spawn({
        let shutdown_token = shutdown_token.clone();
        async move {
            tokio::signal::ctrl_c().await.expect("Failed to catch Ctrl+C");
            info!("Shutdown signal received, initiating graceful shutdown");
            shutdown_token.cancel();
        }
    });

    let auth_service = AuthService::new(store.clone(), geoip, telegram_service.telegram().clone(), templates.clone());

    info!("Starting services: telegram={} auth={}", config.bind_telegram, config.bind_auth);

    match tokio::try_join!(
        telegram_service.serve(&config.bind_telegram, shutdown_token.clone()),
        auth_service.serve(&config.bind_auth, shutdown_token.clone())
    ) {
        Ok(_) => {}
        Err(_) => {
            return ExitCode::FAILURE;
        }
    }

    info!("Bye-bye!");

    ExitCode::SUCCESS
}
