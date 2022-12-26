use crate::cli::constants::DEFAULT_BUF_SIZE;
use crate::cli::Backup;
use crate::cli::Cli;
use crate::compression::CompressionType;
use crate::core::path::BackupPathComponents;
use crate::core::Split;
use crate::crypto::openpgp::openpgp_error;
use crate::crypto::openpgp::Keyring;

use sequoia_openpgp::policy::Policy;
use sequoia_openpgp::policy::StandardPolicy;
use sequoia_openpgp::serialize::stream::Encryptor;
use sequoia_openpgp::serialize::stream::LiteralWriter;
use sequoia_openpgp::serialize::stream::Message;
use sequoia_openpgp::types::DataFormat;
use sequoia_openpgp::types::SymmetricAlgorithm;
use sequoia_openpgp::Cert;

use std::fs;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};

pub fn perform_backup(cli: &Cli, backup: &Backup) -> io::Result<()> {
    log::info!("BACKUP…");

    let backup_path_components: BackupPathComponents = (
        cli.spool.clone(),
        backup.vault,
        backup.output.clone(),
        time::OffsetDateTime::now_utc(),
    )
        .into();

    let backup_dir: Option<PathBuf> = (&backup_path_components).into();
    let Some(backup_dir) = backup_dir else {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("Invalid path {backup_path_components:?} given")));
    };

    let mut recipients: Vec<Box<dyn age::Recipient>> = vec![];
    if backup.recipient.is_some() {
        for recipient in backup.recipient.as_ref().unwrap() {
            recipients.push(recipient.get_recipient());
        }
    }

    /*
    log::debug!(
        "Age Recipients: {recipients:?}",
        recipients = backup.recipient
    );
    log::debug!("OpenPGP Keyring: {keyring:?}", keyring = backup.keyring);
    */
    if backup.keyring.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Keyring is empty",
        ));
    }

    // get certificates from keyring
    let policy = StandardPolicy::new();
    let cert_list: Keyring = parse_keyring(&policy, backup.keyring.iter().flatten())?;

    // setup backup directory and splitter encryption sink
    // after we have some certificates for storage encryption

    // mkdir -p backup_dir: let the first instance of two concurrent
    // permafrust backup calls win in case they started with the same timestamp
    // https://rcrowley.org/2010/01/06/things-unix-can-do-atomically.html
    crate::use_dir_atomic_create_maybe(&backup_dir, Some(true), Some(true))?;

    // TODO signal handling, Ctrl+C does not finish stream https://rust-cli.github.io/book/in-depth/signals.html
    let mut splitter = Split::new(backup_dir, backup.size);

    let mut encryptor_sink = build_encryptor(cert_list, &mut splitter)?;

    // setup input after we created the backup directory and setup encryption to prevent
    // reading streams (or fifo files) that cannot be written later
    let reader: Box<dyn io::Read> = build_reader(backup.input.as_ref())?;
    let mut buffered_reader = io::BufReader::new(reader);

    log::trace!("Starting backup…");

    let copy_result = match backup.compression {
        CompressionType::None => io::copy(&mut buffered_reader, &mut encryptor_sink)?,
        CompressionType::Zstd => thread_io::write::writer(
            DEFAULT_BUF_SIZE,
            1,
            &mut encryptor_sink,
            |writer| -> io::Result<u64> {
                let mut zstd_encoder = zstd::stream::Encoder::new(writer, 0)?;
                let result = compressor_worker(&mut buffered_reader, &mut zstd_encoder);
                if result.is_ok() {
                    zstd_encoder.do_finish()?
                }
                result
            },
        )?,
        CompressionType::Lz4 => thread_io::write::writer(
            DEFAULT_BUF_SIZE,
            1,
            &mut encryptor_sink,
            |writer| -> io::Result<u64> {
                let mut lz4_encoder = lz4_flex::frame::FrameEncoder::new(writer);
                let result = compressor_worker(&mut buffered_reader, &mut lz4_encoder);
                if result.is_ok() {
                    lz4_encoder.try_finish()?
                }
                result
            },
        )?,
    };

    log::trace!("Wrote total of {copy_result} bytes");
    encryptor_sink.flush()?;
    encryptor_sink.finalize().map_err(openpgp_error)
}

fn compressor_worker(reader: &mut dyn io::Read, compressor: &mut dyn io::Write) -> io::Result<u64> {
    log::trace!("Starting compressor worker…");
    io::copy(reader, compressor)
}

fn parse_keyring<'a, K>(policy: &'a dyn Policy, keyring: K) -> io::Result<Keyring<'a>>
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

fn build_encryptor<'a, R, W: 'a + io::Write + Send + Sync>(
    recipients: R,
    output: W,
) -> io::Result<Message<'a>>
where
    R: IntoIterator,
    R::Item: Into<sequoia_openpgp::serialize::stream::Recipient<'a>>,
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

fn build_reader(path: Option<&PathBuf>) -> io::Result<Box<dyn io::Read>> {
    let reader: Box<dyn io::Read> = match path {
        Some(p) if p.as_path() == Path::new("-") => {
            log::info!("Reading from stdin…");
            Box::new(io::stdin())
        }
        None => {
            log::info!("Reading from stdin…");
            Box::new(io::stdin())
        }
        Some(input) => {
            log::info!("Opening {input:?}…");
            Box::new(fs::File::open(input)?)
        }
    };
    Ok(reader)
}
