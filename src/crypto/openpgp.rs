use sequoia_openpgp as openpgp;
use std::io;

use openpgp::{
    cert::prelude::ValidKeyAmalgamation,
    packet::key::{PublicParts, UnspecifiedRole},
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
                "Encrypting for certificate {storage_cert} subkey {subkey} {algo}{size}",
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
    log::trace!("Setting up encryption…");
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
