use crate::cli::Backup;
use crate::cli::Cli;
use crate::compression::CompressionType;
use crate::core::Split;
use crate::crypto::openpgp::openpgp_error;
use crate::crypto::openpgp::Keyring;
use age::Recipient;
use sequoia_openpgp::policy::StandardPolicy;
use sequoia_openpgp::serialize::stream::Encryptor;
use sequoia_openpgp::serialize::stream::LiteralWriter;
use sequoia_openpgp::serialize::stream::Message;
use sequoia_openpgp::types::DataFormat;
use sequoia_openpgp::types::SymmetricAlgorithm;

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

pub fn perform_backup(cli: &Cli, backup: &Backup) -> io::Result<()> {
    log::info!("BACKUP...");

    let mut backup_dir = PathBuf::new();

    // backup_dir starts with the spool directory
    let spool = &cli.spool;
    backup_dir.push(spool);

    // next we add a vault as lower-case hyphenated UUID
    let backup_vault_string = backup.vault.to_string();
    let backup_vault_path = Path::new(&backup_vault_string);
    let vault_dir = build_canonical_path(backup_vault_path)?;
    log::trace!("Using vault directory {vault_dir:?}");
    backup_dir.push(vault_dir);

    // then the output key, potentially containing a path of length >= 1
    let output: &Path = match &backup.output {
        None => Path::new(""),
        Some(output) => output.as_path(),
    };

    let output_dir = build_canonical_path(output)?;
    log::trace!("Using output directory {output_dir:?}");
    backup_dir.push(output_dir);

    // finally, the current UTC timestamp
    let utc_now = time::OffsetDateTime::now_utc();
    let utc_string = utc_now.unix_timestamp().to_string();
    let utc_timestamp = Path::new(&utc_string);

    let timestamp_dir = build_canonical_path(utc_timestamp)?;
    log::trace!("Using timestamp directory {timestamp_dir:?}");
    backup_dir.push(timestamp_dir);

    let mut recipients: Vec<Box<dyn Recipient>> = vec![];
    if backup.recipient.is_some() {
        for recipient in backup.recipient.as_ref().unwrap() {
            recipients.push(recipient.get_recipient());
        }
    }

    log::debug!(
        "Age Recipients: {recipients:?}",
        recipients = backup.recipient
    );
    log::debug!("OpenPGP Keyring: {keyring:?}", keyring = backup.keyring);

    if backup.keyring.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Keyring is empty",
        ));
    }

    // get certificates from keyring
    let policy = StandardPolicy::new();
    let mut cert_list: Keyring = Vec::new();
    for cert in &backup.keyring {
        for storage in cert
            .keys()
            .with_policy(&policy, None)
            .supported()
            .alive()
            .revoked(false)
            .for_storage_encryption()
        {
            log::info!(
                "Encrypting for storage certificate {storage_cert:?}",
                storage_cert = storage.cert().fingerprint()
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

    // setup backup directory and splitter encryption sink
    // after we have some certificates for storage encryption

    // mkdir -p backup_dir: let the first instance of two concurrent
    // permafrust backup calls win in case they started with the same timestamp
    // https://rcrowley.org/2010/01/06/things-unix-can-do-atomically.html
    crate::use_dir_atomic_create_maybe(&backup_dir, Some(true), Some(true))?;

    // TODO signal handling, Ctrl+C does not finish stream https://rust-cli.github.io/book/in-depth/signals.html
    let splitter = Split::new(backup_dir, backup.size);

    let message = Message::new(splitter);

    let encryptor =
        Encryptor::for_recipients(message, cert_list).symmetric_algo(SymmetricAlgorithm::AES256);

    // Encrypt the message.
    let message = encryptor.build().map_err(openpgp_error)?;

    // Literal wrapping.
    let mut message = LiteralWriter::new(message)
        .format(DataFormat::Binary)
        .build()
        .map_err(openpgp_error)?;

    /*
    let mut writer = match backup.compression {
        CompressionType::Zstd => {
            let zstd_encoder = zstd::stream::Encoder::new(encryptor, 0)?;
            FinalEncoder::new(Box::new(zstd_encoder))
        }
        CompressionType::Lz4 => {
            let lz4_encoder = lz4_flex::frame::FrameEncoder::new(encryptor);
            FinalEncoder::new(Box::new(lz4_encoder))
        }
        CompressionType::None => FinalEncoder::new(Box::new(encryptor)),
    }; */

    // setup input after we created the backup directory and setup encryption to prevent
    // reading streams (or fifo files) that cannot be written later
    let reader: Box<dyn io::Read> = build_reader(backup.input.as_ref())?;
    let mut buffered_reader = io::BufReader::new(reader);

    log::trace!("Starting backup ...");
    let copy_result = io::copy(&mut buffered_reader, &mut message)?;
    log::trace!("Wrote {copy_result} bytes");

    message.finalize().map_err(openpgp_error)
}

fn build_reader(path: Option<&PathBuf>) -> io::Result<Box<dyn io::Read>> {
    let reader: Box<dyn io::Read> = match path {
        Some(p) if p.as_path() == Path::new("-") => {
            log::info!("Reading from stdin ...");
            Box::new(io::stdin())
        }
        None => {
            log::info!("Reading from stdin ...");
            Box::new(io::stdin())
        }
        Some(input) => {
            log::info!("Opening {input:?} ...");
            Box::new(fs::File::open(input)?)
        }
    };
    Ok(reader)
}

fn build_canonical_path(dir: &Path) -> io::Result<PathBuf> {
    //if dir.is_empty() {
    //    return Err(io::Error::new(
    //        io::ErrorKind::InvalidInput,
    //        "Empty path given",
    //    ));
    // }

    let mut canonical_dir_path = PathBuf::new();

    // create canonical representation
    for component in dir.components() {
        match component {
            Component::Normal(subpath) => {
                canonical_dir_path.push(subpath);
            }
            Component::CurDir => {
                // ignore
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Invalid path {dir:?} given"),
                ));
            }
        }
    }

    Ok(canonical_dir_path)
}
