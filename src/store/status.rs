use fred::error::{Error as FredError, ErrorKind as FredErrorKind};
use fred::types::{FromValue as FredFromValue, Value as FredValue};
use std::fmt;
use std::str::FromStr;

impl FredFromValue for AuthStatus {
    fn from_value(value: FredValue) -> Result<Self, FredError> {
        let s = value.into_string().ok_or(FredError::new(FredErrorKind::Parse, "Could not convert to status."))?;
        AuthStatus::from_str(&s).map_err(|_| FredError::new(FredErrorKind::Parse, "Unknown status variant"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthStatus {
    Deny,
    Allow,
    Pending,
}

impl FromStr for AuthStatus {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "allow" => Ok(AuthStatus::Allow),
            "deny" => Ok(AuthStatus::Deny),
            "pending" => Ok(AuthStatus::Pending),
            _ => Err("Unknown status variant"),
        }
    }
}

impl fmt::Display for AuthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            AuthStatus::Deny => "deny",
            AuthStatus::Allow => "allow",
            AuthStatus::Pending => "pending",
        };
        write!(f, "{}", s)
    }
}

impl AsRef<str> for AuthStatus {
    fn as_ref(&self) -> &str {
        match self {
            AuthStatus::Allow => "allow",
            AuthStatus::Deny => "deny",
            AuthStatus::Pending => "pending",
        }
    }
}
