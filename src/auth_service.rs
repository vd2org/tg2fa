use crate::store::Store;
use crate::store::ValidateResult;

use axum::extract::State;

use crate::geoip_service::Geoip;
use crate::templates::Templates;
use crate::tools::token_auth;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use log::{debug, error, info, trace};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tgrs::{Telegram, TelegramRequestResponse};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SecondFactorData {
    pub id: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MetaData {
    pub username: String,
    pub device_name: String,
    pub ip_addr: IpAddr,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AuthRequest {
    pub id: String,
    pub meta_data: MetaData,
    pub second_factor_data: SecondFactorData,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AuthValidate {
    pub id: String,
    pub code: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AuthResponse {
    ok: bool,
    description: Option<String>,
}
#[derive(Clone)]
pub struct AuthService {
    store: Store,
    geoip: Option<Arc<Geoip>>,
    telegram: Telegram,
    templates: Templates,
}

impl AuthService {
    pub fn new(store: Store, geoip: Option<Arc<Geoip>>, telegram: Telegram, templates: Templates) -> Self {
        AuthService { store, geoip, telegram, templates }
    }

    pub async fn serve(
        &self,
        socket_addr: &SocketAddr,
        shutdown_token: CancellationToken,
    ) -> Result<(), Box<dyn Error>> {
        let listener = TcpListener::bind(socket_addr)
            .await
            .inspect_err(|e| error!("Failed to bind to auth server address: {}", e))?;

        info!("Auth Server running on http://{}", socket_addr);

        axum::serve(
            listener,
            Router::new()
                .route("/request", post(Self::request_handler))
                .route("/validate", post(Self::validate_handler))
                .with_state(self.clone()),
        )
        .with_graceful_shutdown(async move {
            shutdown_token.cancelled().await;
        })
        .await
        .inspect_err(|e| error!("Telegram server error: {}", e))?;

        Ok(())
    }

    async fn request_handler(
        State(state): State<Self>,
        Json(request): Json<AuthRequest>,
    ) -> (StatusCode, Json<AuthResponse>) {
        debug!(
            "Auth request received: id={} user={} device={} ip={}",
            request.id, request.meta_data.username, request.meta_data.device_name, request.meta_data.ip_addr
        );

        let ip = request.meta_data.ip_addr.to_string();
        let geo_ip = state.geoip.as_deref().map(|geoip| geoip.lookup_ok(&ip)).unwrap_or_default();

        let code = token_auth(6); //TBDSD: consider it configurable

        let req = state.templates.new_request(&request, &geo_ip);

        let message_id = match req.send(&state.telegram).await {
            Ok(message) => {
                trace!(
                    "Sent Telegram message id {} for request id {} to chat {}",
                    message.message_id,
                    request.id,
                    request.second_factor_data.id
                );
                message.message_id
            }
            Err(e) => {
                error!("Failed to send Telegram message for request id {}: {:?}", request.id, e);
                return (
                    StatusCode::OK,
                    AuthResponse { ok: false, description: Some("Unable to send a message!".to_string()) }.into(),
                );
            }
        };

        let reference = format!("{}-{}", request.second_factor_data.id, message_id);
        trace!(
            "Created reference {} for request id {} (second_factor_id={}, message_id={})",
            reference,
            request.id,
            request.second_factor_data.id,
            message_id
        );

        match state.store.new_request(&request, &code, &reference).await {
            Ok(_) => {
                trace!(
                    "Persisted new request id {} reference {} (second_factor_id={}, chat_message_id={})",
                    request.id,
                    reference,
                    request.second_factor_data.id,
                    message_id
                );
            }
            Err(e) => {
                error!("Failed to persist new request id {} reference {}: {:?}", request.id, reference, e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AuthResponse { ok: false, description: Some("Internal error".to_string()) }.into(),
                );
            }
        }

        (StatusCode::OK, AuthResponse { ok: true, description: None }.into())
    }

    async fn validate_handler(
        State(state): State<Self>,
        Json(request): Json<AuthValidate>,
    ) -> (StatusCode, Json<AuthResponse>) {
        debug!("Auth validate received: id={}", request.id);

        match state.store.validate_by_id(&request.id, &request.code).await {
            Ok(ValidateResult::Allowed) => {
                debug!("Validation allowed for id {}", request.id);
                (StatusCode::OK, AuthResponse { ok: true, description: None }.into())
            }
            Ok(ValidateResult::Expired) => {
                debug!("Validation expired or not found for id {}", request.id);
                (
                    StatusCode::OK,
                    AuthResponse { ok: false, description: Some("Expired or not found".to_string()) }.into(),
                )
            }
            Ok(ValidateResult::Denied) => {
                debug!("Validation denied for id {}", request.id);
                (StatusCode::OK, AuthResponse { ok: false, description: Some("Denied".to_string()) }.into())
            }
            Ok(ValidateResult::WrongCode) => {
                debug!("Validation failed for id {}: wrong code", request.id);
                (StatusCode::OK, AuthResponse { ok: false, description: Some("Wrong code".to_string()) }.into())
            }
            Err(e) => {
                error!("Unable to validate request id {}: {:?}", request.id, e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AuthResponse { ok: false, description: Some("Internal error".to_string()) }.into(),
                )
            }
        }
    }
}
