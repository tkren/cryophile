// Copyright The Cryophile Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use crate::cli::Backup;
use crate::compression::CompressionType;
use crate::core::backup_id::BackupId;
use crate::core::constants::{CHUNK_FILE_MODE, CHUNK_FILE_PREFIX, DEFAULT_BUF_SIZE};
use crate::core::path::{CreateDirectory, Queue, SpoolPathComponents};
use crate::core::Split;
use crate::crypto::openpgp::{build_encryptor, openpgp_error, storage_encryption_certs, Keyring};
use crate::Config;

use sequoia_openpgp::policy::StandardPolicy;
use ulid::Ulid;

use std::fs;
use std::io::{self, Write};
use std::os::unix::prelude::OpenOptionsExt;
use std::path::{Path, PathBuf};

// https://github.com/rust-lang/rust-clippy/issues/11631 breaks unwrap_or_else(Ulid::new)
#[allow(clippy::unwrap_or_default)]
pub fn perform_backup(config: &Config, backup: &Backup) -> io::Result<()> {
    let prefix_str_maybe = backup.prefix.as_ref().and_then(|path| path.to_str());
    let backup_id = BackupId::new(
        backup.vault,
        prefix_str_maybe,
        backup.ulid.or(backup.timestamp).unwrap_or_else(Ulid::new),
    );

    let spool_path_components = SpoolPathComponents::new(config.cli.spool.clone(), backup_id);
    let backup_dir =
        spool_path_components.with_queue_path(Queue::Backup, CreateDirectory::Recursive)?;
    let freeze_dir =
        spool_path_components.with_queue_path(Queue::Freeze, CreateDirectory::Recursive)?;

    #[cfg(feature = "age")]
    {
        let mut recipients: Vec<Box<dyn age::Recipient>> = vec![];
        if backup.recipient.is_some() {
            for recipient in backup.recipient.as_ref().expect("no recipient") {
                recipients.push(recipient.get_recipient());
            }
        }
        log::debug!(
            "Age Recipients: {recipients:?}",
            recipients = backup.recipient
        );
    }

    if backup.keyring.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Keyring is empty",
        ));
    }
    log::debug!(
        "OpenPGP keyring has {num:?} certificate(s)",
        num = backup.keyring.len()
    );

    // get certificates from keyring
    let policy = StandardPolicy::new();
    let cert_list: Keyring = storage_encryption_certs(&policy, backup.keyring.iter().flatten())?;

    // setup backup directory and splitter encryption sink
    // after we have some certificates for storage encryption

    // TODO signal handling, Ctrl+C does not finish stream https://rust-cli.github.io/book/in-depth/signals.html
    let mut splitter = Split::new(&backup_dir, &freeze_dir, CHUNK_FILE_PREFIX, backup.size);

    let mut encryptor_sink = build_encryptor(cert_list, &mut splitter)?;

    // setup input after we created the backup directory and setup encryption to prevent
    // reading streams (or fifo files) that cannot be written later
    let reader: Box<dyn io::Read> = build_reader(backup.input.as_ref())?;
    let mut buffered_reader = io::BufReader::new(reader);

    let backup_uri = spool_path_components
        .uri()
        .expect("cannot create backup uri");
    log::debug!("Starting backup {backup_uri}");

    let copy_result = match backup.compression {
        CompressionType::None => {
            log::info!("Using no compression…");
            io::copy(&mut buffered_reader, &mut encryptor_sink)?
        }
        CompressionType::Zstd => {
            log::info!("Using Zstandard compression…");
            thread_io::write::writer(
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
            )?
        }
        CompressionType::Lz4 => {
            log::info!("Using LZ4 compression…");
            thread_io::write::writer(
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
            )?
        }
    };

    log::debug!("Wrote total of {copy_result} bytes");
    encryptor_sink.flush()?;
    encryptor_sink.finalize().map_err(openpgp_error)?;
    drop(splitter);
    touch_zero_file(&backup_dir, &freeze_dir)?;

    log::info!("Queued backup {backup_uri} for freeze {freeze_dir:?}");
    Ok(())
}

fn touch_zero_file(incoming: &Path, outgoing: &Path) -> io::Result<()> {
    let zero_file = incoming.join(CHUNK_FILE_PREFIX).with_extension("0");
    log::trace!("Touch {zero_file:?}");
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(CHUNK_FILE_MODE)
        .open(&zero_file)?;
    let zero_link = outgoing.join(CHUNK_FILE_PREFIX).with_extension("0");
    log::trace!("Link {zero_file:?}");
    fs::hard_link(zero_file, zero_link)
}

fn compressor_worker(reader: &mut dyn io::Read, compressor: &mut dyn io::Write) -> io::Result<u64> {
    log::trace!("Starting compressor worker…");
    io::copy(reader, compressor)
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
