// Copyright The Cryophile Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use anyhow::Context;
use sequoia_openpgp as openpgp;
use std::{
    collections::HashMap,
    fs::File,
    io::{self, BufReader},
    os::fd::FromRawFd,
};

use openpgp::{
    cert::prelude::ValidKeyAmalgamation,
    crypto::{Decryptor, KeyPair, Password, SessionKey},
    packet::{
        key::{PublicParts, SecretParts, UnspecifiedRole},
        Key, PKESK, SKESK,
    },
    parse::{
        stream::{self, DecryptionHelper, DecryptorBuilder, MessageStructure, VerificationHelper},
        Parse,
    },
    policy::Policy,
    serialize::stream::{Encryptor2, LiteralWriter, Message, Recipient},
    types::{DataFormat, SymmetricAlgorithm},
    Cert, Fingerprint, KeyID,
};

use crate::core::constants::DEFAULT_BUF_SIZE;

pub type Keyring<'a> = Vec<ValidKeyAmalgamation<'a, PublicParts, UnspecifiedRole, bool>>;

pub fn openpgp_error(error: anyhow::Error) -> io::Error {
    let mut reason = String::new();
    for cause in error.chain() {
        reason.push_str(cause.to_string().as_str());
        if cause.source().is_some() {
            reason.push_str(": ");
        }
    }
    if let Ok(err) = error.downcast::<io::Error>() {
        io::Error::new(err.kind(), reason)
    } else {
        io::Error::other(reason)
    }
}

pub fn storage_encryption_certs<'a, K>(
    policy: &'a dyn Policy,
    keyring: K,
) -> io::Result<Keyring<'a>>
where
    K: Iterator<Item = &'a Cert>,
{
    log::trace!("Searching certificates for data-at-rest encryption…");
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
            let algo = mpis.algo().expect("cannot handle unknown algorithm");
            let size = mpis.bits().unwrap_or(0);
            log::info!(
                "Recipient certificate {storage_cert} subkey {subkey} algo {algo}{size}",
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

pub trait PrivateKey {
    fn unlock(&mut self, password: Option<&Password>) -> openpgp::Result<Box<dyn Decryptor>>;
}

struct LocalPrivateKey {
    key: Key<SecretParts, UnspecifiedRole>,
}

impl LocalPrivateKey {
    fn new(key: Key<SecretParts, UnspecifiedRole>) -> Self {
        Self { key }
    }
}

impl PrivateKey for LocalPrivateKey {
    fn unlock(&mut self, password: Option<&Password>) -> openpgp::Result<Box<dyn Decryptor>> {
        let box_decryptor = |kp: KeyPair| -> Box<dyn Decryptor> { Box::new(kp) };
        if self.key.secret().is_encrypted() {
            let pk_algo = self.key.pk_algo();
            let keyid = self.key.keyid();
            let encrypted_key = self.key.secret_mut();
            if password.is_none() {
                // TODO CRYOPHILE_ASKPASS
                // TODO batch mode
                let p: Password =
                    rpassword::prompt_password(format!("Enter password to decrypt key {keyid}: "))?
                        .into();
                encrypted_key.decrypt_in_place(pk_algo, &p)?;
            } else {
                encrypted_key.decrypt_in_place(pk_algo, password.unwrap())?;
            }
        }
        self.key.clone().into_keypair().map(box_decryptor)
    }
}

pub struct SecretKeyStore {
    secret_keys: HashMap<KeyID, Box<dyn PrivateKey>>,
    key_identities: HashMap<KeyID, Fingerprint>,
    password: Option<Password>,
}

impl SecretKeyStore {
    pub fn new(
        secret_keys: HashMap<KeyID, Box<dyn PrivateKey>>,
        key_identities: HashMap<KeyID, Fingerprint>,
        password: Option<Password>,
    ) -> Self {
        Self {
            secret_keys,
            key_identities,
            password,
        }
    }
}

pub fn secret_key_store<'a, K>(
    policy: &'a dyn Policy,
    keyring: K,
    password: Option<Password>,
) -> io::Result<SecretKeyStore>
where
    K: Iterator<Item = &'a Cert>,
{
    log::trace!("Searching secret keys for data-at-rest decryption…");

    let mut keys: HashMap<KeyID, Box<dyn PrivateKey>> = HashMap::new();
    let mut identities: HashMap<KeyID, Fingerprint> = HashMap::new();

    for tsk in keyring {
        for ka in tsk
            .keys()
            .with_policy(policy, None)
            .for_storage_encryption()
        {
            let id: KeyID = ka.key().fingerprint().into();
            let key = if let Ok(private_key) = ka.key().parts_as_secret() {
                let encryption_status = if private_key.has_unencrypted_secret() {
                    "unencrypted"
                } else {
                    "encrypted"
                };
                log::info!("Using {encryption_status} secret key {id} for data-at-rest decryption");
                Box::new(LocalPrivateKey::new(private_key.clone()))
            } else {
                log::warn!("Cert {id} does not contain secret keys");
                continue;
            };
            keys.insert(id.clone(), key);
            identities.insert(id.clone(), tsk.fingerprint());
        }
    }

    if keys.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Keyring does not contain storage encryption keys",
        ));
    }

    Ok(SecretKeyStore::new(keys, identities, password))
}

pub fn build_encryptor<'a, R, W: 'a + io::Write + Send + Sync>(
    recipients: R,
    output: W,
) -> io::Result<Message<'a>>
where
    R: IntoIterator,
    R::Item: Into<Recipient<'a>>,
{
    log::info!(
        "Setting up encryption with {algo}…",
        algo = SymmetricAlgorithm::AES256
    );
    let message = Message::new(output);
    let encryptor =
        Encryptor2::for_recipients(message, recipients).symmetric_algo(SymmetricAlgorithm::AES256);

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

impl VerificationHelper for SecretKeyStore {
    fn get_certs(&mut self, _ids: &[openpgp::KeyHandle]) -> openpgp::Result<Vec<Cert>> {
        Ok(Vec::new()) // Feed the Certs to the verifier here...
    }

    fn check(&mut self, _structure: MessageStructure) -> openpgp::Result<()> {
        Ok(()) // Implement your verification policy here.
    }
}

impl DecryptionHelper for SecretKeyStore {
    fn decrypt<D>(
        &mut self,
        pkesks: &[PKESK],
        _skesks: &[SKESK],
        sym_algo: Option<SymmetricAlgorithm>,
        mut decrypt: D,
    ) -> openpgp::Result<Option<openpgp::Fingerprint>>
    where
        D: FnMut(SymmetricAlgorithm, &SessionKey) -> bool,
    {
        for (num, pkesk) in pkesks.iter().enumerate() {
            let keyid = pkesk.recipient();
            log::trace!("Trying to decrypt session key {num} for recipient {keyid}…");
            if let Some(pair) = self.secret_keys.get_mut(keyid) {
                let mut dec = pair.unlock(self.password.as_ref())?;
                let decryptor = dec.as_mut();
                if pkesk
                    .decrypt(decryptor, sym_algo)
                    .map(|(algo, session_key)| decrypt(algo, &session_key))
                    .unwrap_or(false)
                {
                    let fingerprint = self.key_identities.get_mut(keyid).unwrap();
                    let recipient = Some(fingerprint.clone());
                    log::trace!("Decrypted session key {num} for recipient {keyid}");
                    return Ok(recipient);
                }
                log::warn!("Decrypting session key {num} failed for recipient {keyid}");
            } else {
                log::warn!(
                    "Cannot find secret key to decrypt session key {num} for recipient {keyid}"
                );
            }
        }
        let sk_keyids = self
            .secret_keys
            .keys()
            .map(|keyid| keyid.to_string())
            .collect::<Vec<_>>();
        log::error!("Cannot decrypt session keys using any of the secret keys for {sk_keyids:?}");
        Err(anyhow::anyhow!(
            "Cannot decrypt session keys using any of the secret keys"
        ))
    }
}

pub fn read_password_fd(fd: i32) -> Option<Password> {
    log::debug!("Reading password from file descriptor {fd}…");
    let file = unsafe { File::from_raw_fd(fd) };
    let mut reader = BufReader::new(file);
    rpassword::read_password_from_bufread(&mut reader)
        .map(Into::<Password>::into)
        .map_err(|err| {
            log::warn!("Cannot read password from file descriptor {fd}: {err}");
            err
        })
        .ok()
}

pub fn build_decryptor<'a, R: 'a + io::Read + Send + Sync>(
    secret_key_store: SecretKeyStore,
    policy: &'a dyn Policy,
    input: R,
) -> openpgp::Result<stream::Decryptor<'a, SecretKeyStore>> {
    log::trace!("Setting up decryption…");
    let decryptor = DecryptorBuilder::from_reader(input)?
        .buffer_size(DEFAULT_BUF_SIZE) // we do not verify, no need for a larger buffer
        .mapping(false)
        .with_policy(policy, None, secret_key_store)
        .context("Decryption failed")?;

    Ok(decryptor)
}
