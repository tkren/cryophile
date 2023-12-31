// Copyright The Cryophile Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use std::{fmt, str::FromStr};

use thiserror::Error;

#[derive(Clone)]
pub enum RecipientKind {
    X25519Recipient(age::x25519::Recipient),
    SshRecipient(age::ssh::Recipient),
}

#[derive(Clone)]
pub struct RecipientSpec {
    pub key: String,
    pub recipient: RecipientKind,
}

impl RecipientSpec {
    pub fn get_recipient(&self) -> Box<dyn age::Recipient> {
        match &self.recipient {
            RecipientKind::SshRecipient(r) => Box::new(r.clone()),
            RecipientKind::X25519Recipient(r) => Box::new(r.clone()),
        }
    }
}

impl fmt::Display for RecipientSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.key)
    }
}

impl std::fmt::Debug for RecipientSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecipientSpec")
            .field("key", &self.key)
            .finish()
    }
}

#[derive(Error, Debug)]
pub enum ParseRecipientError {
    #[error("Age recipient error: {0}")]
    Recipient(String),
    #[error("SSH Age recipient error: {0}")]
    SshRecipient(String),
    #[error("Unknown Age recipient error: {0}")]
    Unknown(String),
}

impl FromStr for RecipientSpec {
    type Err = ParseRecipientError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let x25519_recipient = s.parse::<age::x25519::Recipient>();
        if let Ok(r) = x25519_recipient {
            let recipient = RecipientSpec {
                key: s.to_string(),
                recipient: RecipientKind::X25519Recipient(r),
            };

            return Ok(recipient);
        }

        let ssh_recipient = s.parse::<age::ssh::Recipient>();
        if let Ok(r) = ssh_recipient {
            let recipient = RecipientSpec {
                key: s.to_string(),
                recipient: RecipientKind::SshRecipient(r),
            };
            return Ok(recipient);
        }

        if let Err(age::ssh::ParseRecipientKeyError::Unsupported(key_type)) = ssh_recipient {
            return Err(ParseRecipientError::SshRecipient(format!(
                "Cannot parse SSH age recipient, unsupported key type {key_type} found: {s}"
            )));
        }

        if let Err(err) = x25519_recipient {
            return Err(ParseRecipientError::Recipient(format!(
                "Cannot parse age recipient {s}: {err}"
            )));
        }

        Err(ParseRecipientError::Unknown(format!(
            "Cannot parse age recipient {s}: unknown error"
        )))
    }
}
