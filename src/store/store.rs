use super::status::AuthStatus;
use crate::auth_service::AuthRequest;
use fred::prelude::*;
use log::{trace, info};
use std::error::Error;
use std::time::Duration;
use url::Url;

#[derive(Clone, Debug)]
pub struct Store {
    client: Client,
    config: StoreConfig,
}

#[derive(Clone, Debug)]
pub struct FindResult {
    pub ip: String,
    pub status: AuthStatus,
    pub reference: Option<String>,
}

#[derive(Clone, Debug)]
pub struct StoreConfig {
    pub request_expiration: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidateResult {
    Allowed,
    Denied,
    WrongCode,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateResult {
    UpdatedWithCode(String),
    Updated,
    Expired,
}

impl Store {
    pub async fn new(valkey: &Url, config: StoreConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let valkey_config = Config::from_url(valkey.as_str())?;

        // Create a builder so a reconnect policy can be applied.
        let mut builder = Builder::from_config(valkey_config);
        // Allow the client to retry the initial connection instead of failing fast.
        builder.with_config(|c| c.fail_fast = false);
        // Use an exponential backoff reconnect policy with unlimited attempts (0 = retry forever).
        // min_delay = 100ms, max_delay = 30_000ms, base = 2
        builder.set_policy(ReconnectPolicy::new_exponential(0, 100, 30_000, 2));

        let client = builder.build()?;
        // Initialize the client. With fail_fast=false and a reconnect policy this will retry until connected.
        client.init().await?;

        Ok(Self { client, config })
    }

    pub fn config(&self) -> &StoreConfig {
        &self.config
    }

    pub async fn new_request(&self, req: &AuthRequest, code: &str, reference: &str) -> Result<(), Box<dyn Error>> {
        let key_ref = format!("ref:{}", reference).to_lowercase();
        let key_req = format!("req:{}", req.id).to_lowercase();
        let key_code = format!("cod:{}", req.id).to_lowercase();
        let exp = Some(Expiration::EX(self.config.request_expiration.as_secs() as i64));

        let trx = self.client.multi();
        trx.set::<(), &str, &str>(&key_ref, &req.id, exp.clone(), None, false).await?;
        trx.set::<(), &str, &str>(&key_req, AuthStatus::Pending.as_ref(), exp.clone(), None, false).await?;
        trx.set::<(), &str, &str>(&key_code, code, exp.clone(), None, false).await?;
        trx.exec::<()>(true).await?;
        trace!("Persisted new request id={} reference={} exp_secs={}", req.id, reference, self.config.request_expiration.as_secs());

        Ok(())
    }

    pub async fn validate_by_id(&self, req_id: &str, code: &str) -> Result<ValidateResult, Box<dyn Error>> {
        let key_code = format!("cod:{}", req_id).to_lowercase();
        let key_req = format!("req:{}", req_id).to_lowercase();

        let values =
            self.client.mget::<(Option<String>, Option<AuthStatus>), _>(&[key_code.as_str(), key_req.as_str()]).await?;

        match values {
            (None, _) | (_, None) => Ok(ValidateResult::Expired),
            (Some(stored_code), Some(AuthStatus::Allow)) if stored_code == code => Ok(ValidateResult::Allowed),
            (Some(stored_code), Some(AuthStatus::Allow)) if stored_code != code => Ok(ValidateResult::WrongCode),
            _ => Ok(ValidateResult::Denied),
        }
    }

    pub async fn update_by_reference(
        &self,
        reference: &str,
        status: AuthStatus,
    ) -> Result<UpdateResult, Box<dyn Error>> {
        trace!("Updating reference {} with status {:?}", reference, status);

        let key_ref = format!("ref:{}", reference).to_lowercase();

        let Some(req_id) = self.client.get::<Option<String>, &str>(&key_ref).await? else {
            trace!("Reference {} not found or expired", reference);
            return Ok(UpdateResult::Expired);
        };

        trace!("Resolved reference {} -> req_id {}", reference, req_id);

        let key_req = format!("req:{}", req_id).to_lowercase();
        let key_code = format!("cod:{}", req_id).to_lowercase();

        let trx = self.client.multi();

        let args = vec![key_req.as_str(), status.as_ref(), "IFEQ", AuthStatus::Pending.as_ref(), "KEEPTTL"];
        trx.custom::<(), _>(fred::cmd!("SET"), args).await?;
        trx.get::<Option<String>, &str>(&key_code).await?;

        match trx.exec::<(Option<()>, Option<String>)>(true).await? {
            (_, None) | (None, _) => {
                trace!("Update by reference {} expired (no req or no code)", reference);
                Ok(UpdateResult::Expired)
            }
            (Some(_), Some(code)) if status == AuthStatus::Allow => {
                trace!("Reference {} updated: Allow (code_len={})", reference, code.len());
                Ok(UpdateResult::UpdatedWithCode(code))
            }
            (Some(_), Some(_)) => {
                trace!("Reference {} updated with status {:?}", reference, status);
                Ok(UpdateResult::Updated)
            }
        }
    }
}
