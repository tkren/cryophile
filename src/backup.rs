use chrono::{DateTime, Utc};

use crate::constants::DEFAULT_COMPRESSION;
use crate::Config;
use crate::FinalEncoder;
use crate::Split;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

pub fn perform_backup(config: Config, matches: &clap::ArgMatches) -> io::Result<()> {
    log::info!("BACKUP...");
    if config.verbose {
        log::debug!("Printing verbose info...");
    } else if !config.quiet {
        log::debug!("Printing normally...");
    }

    let mut backup_dir = PathBuf::new();

    // backup_dir starts with the spool directory
    let spool = &config.spool;
    backup_dir.push(spool);

    // next we add a vault
    let vault_arg = matches.value_of("vault").unwrap_or("");
    let vault_uuid = match uuid::Uuid::parse_str(vault_arg) {
        Ok(uuid) => uuid.to_string(), // lower-case hyphenated UUID

        Err(err) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Invalid vault {vault_arg} given: {err}"),
            ));
        }
    };

    let vault_dir = build_canonical_path(&vault_uuid)?;
    log::trace!("Using vault directory {vault_dir:?}");
    backup_dir.push(vault_dir);

    // then the output key, potentially containing a path of length >= 1
    let output = matches.value_of("output").unwrap_or("");

    let output_dir = build_canonical_path(output)?;
    log::trace!("Using output directory {output_dir:?}");
    backup_dir.push(output_dir);

    // finally, the current UTC timestamp
    let utc_now: DateTime<Utc> = Utc::now();
    let utc_timestamp = utc_now.timestamp().to_string();

    let timestamp_dir = build_canonical_path(&utc_timestamp)?;
    log::trace!("Using timestamp directory {timestamp_dir:?}");
    backup_dir.push(timestamp_dir);

    // mkdir -p backup_dir: let the first instance of two concurrent
    // permafrust backup calls win in case they started with the same timestamp
    // https://rcrowley.org/2010/01/06/things-unix-can-do-atomically.html
    crate::use_dir_atomic_create_maybe(&backup_dir, Some(true), Some(true))?;

    // TODO signal handling, Ctrl+C does not finish stream https://rust-cli.github.io/book/in-depth/signals.html
    let splitter = Split::new(backup_dir, config.chunk_size);

    let mut writer = match matches
        .value_of("compression")
        .unwrap_or_else(|| DEFAULT_COMPRESSION.into())
    {
        "zstd" => {
            let zstd_encoder = zstd::stream::Encoder::new(splitter, 0)?;
            FinalEncoder::new(Box::new(zstd_encoder))
        }
        "lz4" => {
            let lz4_encoder = lz4_flex::frame::FrameEncoder::new(splitter);
            FinalEncoder::new(Box::new(lz4_encoder))
        }
        _ => FinalEncoder::new(Box::new(splitter)),
    };

    // setup input after we created the backup directory to prevent
    // reading streams (or fifo files) that cannot be written later
    let input = matches.value_of("input").unwrap_or("-");

    let reader: Box<dyn io::Read> = match input {
        "-" => {
            log::info!("Reading from stdin ...");
            Box::new(io::stdin())
        }
        _ => {
            log::info!("Opening {input:?} ...");
            Box::new(fs::File::open(input)?)
        }
    };

    let mut buffered_reader = io::BufReader::new(reader);

    io::copy(&mut buffered_reader, &mut writer)?;

    Ok(())
}

fn build_canonical_path(dir: &str) -> io::Result<PathBuf> {
    if dir.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Empty path given",
        ));
    }

    let dir_path = Path::new(dir);
    let mut canonical_dir_path = PathBuf::new();

    // create canonical representation
    for component in dir_path.components() {
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
