use crate::store::AuthStatus;
use crate::store::Store;
use crate::store::UpdateResult;

use axum::http::{HeaderMap, StatusCode};
use tgrs::{AnswerCallbackQuery, TelegramRequestResponse, Update, User, GET_ME};

use crate::tools::{extract_header, token_urlsafe};
use log::{debug, error, info, trace, warn};
use std::error::Error;
use tgrs::SetWebhook;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use crate::templates::{Resolution, Templates};
use axum::body::Body;
use axum::extract::{Json, Request, State};
use axum::middleware;
use axum::middleware::Next;
use axum::response::IntoResponse;
use axum::routing::any;
use constant_time_eq::classic::constant_time_eq;
use std::net::SocketAddr;
use tgrs::{CallbackQuery, Chat, ChatType, Message, Telegram};
use url::Url;

const HEADER_TELEGRAM_TOKEN: &str = "x-telegram-bot-api-secret-token";

#[derive(Clone, Debug)]
pub struct TelegramService {
    store: Store,
    templates: Templates,
    secret_token: String,
    telegram: Telegram,
    username: String,
}

impl TelegramService {
    pub async fn new(
        store: Store,
        templates: Templates,
        token: &str,
        secret_token: Option<String>,
    ) -> Result<Self, Box<dyn Error>> {
        let secret_token = secret_token.unwrap_or_else(|| token_urlsafe(64));
        let telegram = Telegram::new(token)?;

        let username = match GET_ME.send(&telegram).await? {
            User { username: Some(username), .. } => username,
            _ => return Err("Failed to extract bot username from Telegram response".into()),
        };
        trace!("Telegram bot username: {}", username);

        Ok(TelegramService { store, templates, secret_token, telegram, username })
    }

    pub fn telegram(&self) -> &Telegram {
        &self.telegram
    }

    pub async fn setup_webhook(&self, url: &Url) -> Result<(), Box<dyn Error>> {
        let set_webhook = SetWebhook::builder()
            .url(url.as_str())
            .max_connections(5)
            .allowed_updates(vec!["message".into(), "callback_query".into()])
            .drop_pending_updates(true)
            .secret_token(&self.secret_token)
            .build();

        trace!("Setting Telegram webhook to {}", url.as_str());

        let _resp = self.telegram.call_raw(&set_webhook).await?;
        info!("Webhook configured at {} for bot @{}", url.as_str(), self.username());
        Ok(())
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    pub async fn serve(
        &self,
        socket_addr: &SocketAddr,
        shutdown_token: CancellationToken,
    ) -> Result<(), Box<dyn Error>> {
        let listener =
            TcpListener::bind(socket_addr).await.inspect_err(|e| error!("Failed to bind to {}: {}", socket_addr, e))?;

        info!("Telegram Server running on http://{} (bot @{})", socket_addr, self.username());

        let mw = middleware::from_fn_with_state(self.clone(), Self::middleware);
        let service = any(Self::handler).with_state(self.clone()).layer(mw);

        axum::serve(listener, service)
            .with_graceful_shutdown(async move {
                shutdown_token.cancelled().await;
            })
            .await
            .inspect_err(|e| error!("Auth server error on {}: {}", socket_addr, e))?;

        Ok(())
    }

    async fn middleware(State(service): State<Self>, req: Request, next: Next) -> impl IntoResponse {
        let token = extract_header(req.headers(), HEADER_TELEGRAM_TOKEN, "");
        if !constant_time_eq(token.as_bytes(), service.secret_token.as_bytes()) {
            warn!(
                "Secret token mismatch: provided length = {}, expected length = {}",
                token.len(),
                service.secret_token.len()
            );
            return (StatusCode::UNAUTHORIZED, HeaderMap::new(), Body::empty()).into_response();
        };

        next.run(req).await
    }

    async fn handler(State(service): State<Self>, Json(update): Json<Update>) -> StatusCode {
        match update {
            Update {
                callback_query:
                    Some(CallbackQuery {
                        id: query_id,
                        data,
                        message: Some(Message { message_id, chat: Chat { id: chat_id, type_: ChatType::Private } }),
                        ..
                    }),
                ..
            } => {
                trace!(
                    "Received callback query id={} chat_id={} message_id={} data={}",
                    query_id,
                    chat_id,
                    message_id,
                    data
                );
                Self::callback(&service, &query_id, &chat_id, &message_id, &data).await
            }
            Update { message: Some(Message { chat: Chat { id: chat_id, type_: ChatType::Private }, .. }), .. } => {
                trace!("Received message for chat_id={}", chat_id);
                Self::message(&service, &chat_id).await
            }
            update => {
                warn!("Unsupported update payload from Telegram: {:?}", update);
                StatusCode::OK
            }
        }
    }

    async fn callback(service: &Self, id: &str, chat_id: &i64, message_id: &u64, data: &str) -> StatusCode {
        let id_owned = id.to_string();
        tokio::spawn({
            let telegram = service.telegram.clone();
            let id_owned = id_owned;
            async move {
                let ans = AnswerCallbackQuery::simple(&id_owned);
                match ans.send(&telegram).await {
                    Ok(_) => trace!("Answered callback query id {}", id_owned),
                    Err(e) => warn!("Error sending AnswerCallbackQuery id {}: {}", id_owned, e),
                };
            }
        });

        tokio::spawn({
            let telegram = service.telegram.clone();
            let store = service.store.clone();
            let templates = service.templates.clone();
            let data = data.to_string();
            let chat_id = chat_id.clone();
            let message_id = message_id.clone();
            async move {
                let status = match data.as_str().trim() {
                    "allow" => AuthStatus::Allow,
                    "deny" => AuthStatus::Deny,
                    "noop" => {
                        trace!("Noop callback command for chat={}, message={}", chat_id, message_id);
                        return;
                    }
                    _ => {
                        warn!(
                            "Unknown command in callback data: '{}' (chat={}, message={})",
                            data, chat_id, message_id
                        );
                        return;
                    }
                };

                let reference = format!("{}-{}", chat_id, message_id);
                trace!("Updating reference {} with status {:?}", reference, status);

                let resolution = match store.update_by_reference(&reference, status).await {
                    Ok(UpdateResult::UpdatedWithCode(code)) if status == AuthStatus::Allow => Resolution::Allow(code),
                    Ok(UpdateResult::Updated) if status == AuthStatus::Deny => Resolution::Deny,
                    Ok(UpdateResult::Expired) => Resolution::Expired,
                    Ok(result) => {
                        // This should never happen, indicating an unexpected state in the store!
                        error!(
                            "Unexpected update result for reference {} (chat={}, msg={}): status={:?}, result={:?}",
                            reference, chat_id, message_id, status, result
                        );
                        return;
                    }
                    Err(e) => {
                        error!(
                            "Failed to update reference {} (chat={}, msg={}, status={:?}): {}",
                            reference, chat_id, message_id, status, e
                        );
                        return;
                    }
                };

                match &resolution {
                    Resolution::Allow(code) => debug!(
                        "Authorization allowed for reference {} (chat={}, msg={}, code_len={})",
                        reference,
                        chat_id,
                        message_id,
                        code.len()
                    ),
                    Resolution::Deny => {
                        debug!("Authorization denied for reference {} (chat={}, msg={})", reference, chat_id, message_id)
                    }
                    Resolution::Expired => debug!(
                        "Authorization expired for reference {} (chat={}, msg={})",
                        reference, chat_id, message_id
                    ),
                }

                let edit = templates.update_request(chat_id, message_id, &resolution);

                match edit.send(&telegram).await {
                    Ok(_) => debug!("Edited message {} in chat {} after processing callback", message_id, chat_id),
                    Err(e) => error!(
                        "Failed to edit message {} in chat {} after update ({:?}): {}",
                        message_id, chat_id, status, e
                    ),
                }
            }
        });

        StatusCode::OK
    }

    async fn message(service: &Self, chat_id: &i64) -> StatusCode {
        trace!("Sending user_id template to chat {}", chat_id);
        let chat_id_owned = *chat_id;
        tokio::spawn({
            let telegram = service.telegram.clone();
            let msg = service.templates.user_id(chat_id_owned);
            async move {
                match msg.send(&telegram).await {
                    Ok(sent) => trace!("Sent user_id message to chat {} message_id {}", chat_id_owned, sent.message_id),
                    Err(e) => warn!("Unable to send message to chat {}: {}", chat_id_owned, e),
                };
            }
        });

        StatusCode::OK
    }
}
