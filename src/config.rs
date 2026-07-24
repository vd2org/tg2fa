use clap::Parser;
use humantime::Duration;
use std::net::SocketAddr;
use std::path::PathBuf;
use url::Url;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about)]
pub struct AppConfig {
    #[arg(long, env = "TELEGRAM_BOT_TOKEN")]
    pub telegram_bot_token: String,

    #[arg(long, env = "TELEGRAM_HOST_URL")]
    pub telegram_host_url: Url,

    #[arg(long, env = "CONFIG_REQUEST_EXPIRATION", default_value = "5m")]
    pub config_request_expiration: Duration,

    #[arg(long, env = "GEOIP_CITY", default_value = "/opt/geoip/city.mmdb")]
    pub geoip_city: PathBuf,

    #[arg(long, env = "GEOIP_ASN", default_value = "/opt/geoip/asn.mmdb")]
    pub geoip_asn: PathBuf,

    #[arg(long, env = "VALKEY_SERVER", default_value = "redis://127.0.0.1:6379")]
    pub valkey_server: Url,

    #[arg(long, env = "BIND_AUTH", default_value = "0.0.0.0:9090")]
    pub bind_auth: SocketAddr,

    #[arg(long, env = "BIND_TELEGRAM", default_value = "0.0.0.0:8080")]
    pub bind_telegram: SocketAddr,
}
