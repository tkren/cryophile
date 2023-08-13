// Copyright The Permafrust Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use crate::crypto::age::RecipientSpec;
use crate::crypto::openpgp::openpgp_error;
use sequoia_openpgp::cert::CertParser;
use sequoia_openpgp::parse::Parse;
use sequoia_openpgp::Cert;

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
