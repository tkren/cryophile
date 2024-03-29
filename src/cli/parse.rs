// Copyright The Cryophile Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use std::path::PathBuf;
use std::str::FromStr;

#[cfg(feature = "age")]
use crate::crypto::age::RecipientSpec;

use crate::crypto::openpgp::openpgp_error;
use chrono::{DateTime, FixedOffset};
use sequoia_openpgp::cert::CertParser;
use sequoia_openpgp::parse::Parse;
use sequoia_openpgp::Cert;
use ulid::Ulid;

use super::UNSAFE_PREFIX;

pub(crate) fn parse_chunk_size(s: &str) -> Result<usize, String> {
    let parse_config = parse_size::Config::new()
        .with_binary()
        .with_byte_suffix(parse_size::ByteSuffix::Deny);
    let parse_size_result = parse_config
        .parse_size(s)
        .map_err(|e| format!("Cannot parse chunk size: {e}"))?;
    let chunk_size = usize::try_from(parse_size_result)
        .map_err(|e| format!("Cannot parse chunk size (size exceeds usize): {e}"))?;
    Ok(chunk_size)
}

pub(crate) fn parse_uuid(s: &str) -> Result<uuid::Uuid, String> {
    let uuid = uuid::Uuid::parse_str(s).map_err(|e| format!("Cannot parse uuid: {e}"))?;
    Ok(uuid)
}

#[cfg(feature = "age")]
pub(crate) fn parse_recipient(s: &str) -> Result<RecipientSpec, String> {
    let recipient = s
        .parse::<RecipientSpec>()
        .map_err(|e| format!("Cannot parse age: {e}"))?;
    Ok(recipient)
}

pub(crate) fn parse_keyring(s: &str) -> Result<Vec<Cert>, String> {
    let mut cert_list: Vec<Cert> = Vec::new();
    let parser = CertParser::from_file(s).map_err(|e| openpgp_error(e).to_string())?;
    for parsed_cert in parser {
        if let Err(err) = parsed_cert {
            return Err(openpgp_error(err).to_string());
        }
        let result: Cert =
            parsed_cert.expect("parsing errors for certificates should have been caught before");
        cert_list.push(result);
    }
    if cert_list.is_empty() {
        return Err(format!("Keyring {s} is empty"));
    }
    Ok(cert_list)
}

pub(crate) fn parse_fd(s: &str) -> Result<i32, String> {
    let raw_fd = s.parse::<i32>().map_err(|e| e.to_string())?;
    if raw_fd < 0 {
        return Err("Parsed file descriptor is smaller than 0".to_string());
    }
    Ok(raw_fd)
}

pub(crate) fn parse_timestamp_for_ulid(s: &str) -> Result<Ulid, String> {
    let timestamp = s
        .parse::<DateTime<FixedOffset>>()
        .map_err(|e| e.to_string())?;
    Ok(Ulid::from_datetime(timestamp.into()))
}

pub(crate) fn parse_ulid(s: &str) -> Result<Ulid, String> {
    let ulid = Ulid::from_string(s).map_err(|e| format!("Cannot parse ulid: {e}"))?;
    Ok(ulid)
}

pub(crate) fn parse_prefix(s: &str) -> Result<PathBuf, String> {
    if s.is_empty() {
        return Err("prefix cannot be empty".to_string());
    }

    // https://docs.aws.amazon.com/AmazonS3/latest/userguide/object-keys.html
    if let Some(unsafe_match) = UNSAFE_PREFIX.find(s) {
        return Err(format!(
            "prefix must not contain unsafe characters matching {u}, found {m}",
            u = UNSAFE_PREFIX.as_str(),
            m = unsafe_match.as_str(),
        ));
    }

    let path = PathBuf::from_str(s).map_err(|e| e.to_string())?;
    if path.has_root() {
        return Err("prefix cannot have a root component or be absolute".to_string());
    }
    Ok(path)
}

pub(crate) fn parse_spool(s: &str) -> Result<PathBuf, String> {
    if s.is_empty() {
        return Err("spool cannot be empty".to_string());
    }
    let path = PathBuf::from_str(s).map_err(|e| e.to_string())?;
    if path.is_symlink() {
        return Err("spool cannot be a symlink".to_string());
    }
    if !path.is_dir() {
        return Err("spool must be a directory".to_string());
    }
    Ok(path)
}

pub(crate) fn parse_config(s: &str) -> Result<PathBuf, String> {
    if s.is_empty() {
        return Err("config cannot be empty".to_string());
    }
    let path = PathBuf::from_str(s).map_err(|e| e.to_string())?;
    if path.is_dir() {
        return Err("config cannot be a directory".to_string());
    }
    Ok(path)
}
