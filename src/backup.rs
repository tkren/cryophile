use chrono::{DateTime, Utc};

use crate::cli::Backup;
use crate::cli::Cli;
use crate::constants::CompressionType;
use crate::FinalEncoder;
use crate::Split;
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
    let utc_now: DateTime<Utc> = Utc::now();
    let utc_string = utc_now.timestamp().to_string();
    let utc_timestamp = Path::new(&utc_string);

    let timestamp_dir = build_canonical_path(utc_timestamp)?;
    log::trace!("Using timestamp directory {timestamp_dir:?}");
    backup_dir.push(timestamp_dir);

    // mkdir -p backup_dir: let the first instance of two concurrent
    // permafrust backup calls win in case they started with the same timestamp
    // https://rcrowley.org/2010/01/06/things-unix-can-do-atomically.html
    crate::use_dir_atomic_create_maybe(&backup_dir, Some(true), Some(true))?;

    // TODO signal handling, Ctrl+C does not finish stream https://rust-cli.github.io/book/in-depth/signals.html
    let splitter = Split::new(backup_dir, backup.size);

    let mut writer = match backup.compression {
        CompressionType::Zstd => {
            let zstd_encoder = zstd::stream::Encoder::new(splitter, 0)?;
            FinalEncoder::new(Box::new(zstd_encoder))
        }
        CompressionType::Lz4 => {
            let lz4_encoder = lz4_flex::frame::FrameEncoder::new(splitter);
            FinalEncoder::new(Box::new(lz4_encoder))
        }
        CompressionType::None => FinalEncoder::new(Box::new(splitter)),
    };

    // setup input after we created the backup directory to prevent
    // reading streams (or fifo files) that cannot be written later
    let reader: Box<dyn io::Read> = match &backup.input {
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

    let mut buffered_reader = io::BufReader::new(reader);

    io::copy(&mut buffered_reader, &mut writer)?;

    Ok(())
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
