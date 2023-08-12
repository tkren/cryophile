// Copyright The Permafrust Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use anyhow::Context;
use sequoia_openpgp as openpgp;
use std::{io, path::Path};

use openpgp::{
    cert::prelude::ValidKeyAmalgamation,
    crypto::SessionKey,
    packet::{
        key::{PublicParts, UnspecifiedRole},
        PKESK, SKESK,
    },
    parse::{
        stream::{
            DecryptionHelper, Decryptor, DecryptorBuilder, MessageStructure, VerificationHelper,
        },
        Parse,
    },
    policy::Policy,
    serialize::stream::{Encryptor, LiteralWriter, Message, Recipient},
    types::{DataFormat, SymmetricAlgorithm},
    Cert,
};

pub type Keyring<'a> = Vec<ValidKeyAmalgamation<'a, PublicParts, UnspecifiedRole, bool>>;

pub fn openpgp_error(e: anyhow::Error) -> io::Error {
    let e = match e.downcast::<io::Error>() {
        Ok(e) => return e,
        Err(e) => e,
    };
    io::Error::new(io::ErrorKind::Other, format!("OpenPGP error: {e}"))
}

pub fn parse_keyring<'a, K>(policy: &'a dyn Policy, keyring: K) -> io::Result<Keyring<'a>>
where
    K: Iterator<Item = &'a Cert>,
{
    log::trace!("Searching certificates for data at rest encryption…");
    // get certificates from keyring
    let mut cert_list: Keyring = Vec::new();
    for cert in keyring {
        for storage in cert
            .keys()
            .with_policy(policy, None)
            .supported()
            .alive()
            .revoked(false)
            .for_storage_encryption()
        {
            let storage_cert = storage.cert().fingerprint();
            let subkey = storage.keyid();
            let mpis = storage.mpis();
            let algo = mpis.algo().unwrap();
            let size = mpis.bits().unwrap_or(0);
            log::info!(
                "Encrypting for certificate {storage_cert} subkey {subkey} algo {algo}{size}",
                storage_cert = storage_cert.to_string(),
                subkey = subkey.to_string(),
                algo = algo.to_string(),
                size = size
            );
            cert_list.push(storage.clone());
        }
    }

    if cert_list.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Keyring does not contain storage encryption certificates",
        ));
    }

    Ok(cert_list)
}

pub fn build_encryptor<'a, R, W: 'a + io::Write + Send + Sync>(
    recipients: R,
    output: W,
) -> io::Result<Message<'a>>
where
    R: IntoIterator,
    R::Item: Into<Recipient<'a>>,
{
    log::trace!(
        "Setting up encryption with {algo}…",
        algo = SymmetricAlgorithm::AES256
    );
    let message = Message::new(output);
    let encryptor =
        Encryptor::for_recipients(message, recipients).symmetric_algo(SymmetricAlgorithm::AES256);

    // Encrypt the message.
    log::trace!("Starting encryption…");
    let message = encryptor.build().map_err(openpgp_error)?;

    // Literal wrapping.
    log::trace!("Setting up encryption stream…");
    LiteralWriter::new(message)
        .format(DataFormat::Binary)
        .build()
        .map_err(openpgp_error)
}

pub struct Helper {}

impl VerificationHelper for Helper {
    fn get_certs(&mut self, _ids: &[openpgp::KeyHandle]) -> openpgp::Result<Vec<Cert>> {
        Ok(Vec::new()) // Feed the Certs to the verifier here...
    }

    fn check(&mut self, _structure: MessageStructure) -> openpgp::Result<()> {
        Ok(()) // Implement your verification policy here.
    }
}

impl DecryptionHelper for Helper {
    fn decrypt<D>(
        &mut self,
        _pkesks: &[PKESK],
        skesks: &[SKESK],
        _sym_algo: Option<SymmetricAlgorithm>,
        mut decrypt: D,
    ) -> openpgp::Result<Option<openpgp::Fingerprint>>
    where
        D: FnMut(SymmetricAlgorithm, &SessionKey) -> bool,
    {
        let password = &"streng geheim".into();
        let session_key_result = skesks[0].decrypt(password);
        let _ = session_key_result.map(|(algo, session_key)| decrypt(algo, &session_key));
        Ok(None)
    }
}

pub fn build_decryptor<'a, R: 'a + io::Read + Send + Sync>(
    _private_key_store: &Path,
    policy: &'a dyn Policy,
    input: R,
) -> openpgp::Result<Decryptor<'a, Helper>>
where
    R: IntoIterator,
    R::Item: Into<Recipient<'a>>,
{
    // TODO feed private_key_store into Helper
    let helper = Helper {};

    log::trace!("Setting up decryption…");
    let decryptor = DecryptorBuilder::from_reader(input)?
        .mapping(false)
        .with_policy(policy, None, helper)
        .context("Decryption failed")?;

    Ok(decryptor)
}
