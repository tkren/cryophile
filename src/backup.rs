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

    let input = matches.value_of("input").unwrap_or("-");

    let reader: Box<dyn io::Read> = match input {
        "-" => {
            log::info!("Reading from stdin ...");
            Box::new(io::stdin())
        }
        _ => {
            log::info!("Opening `{}' ...", input);
            Box::new(fs::File::open(input)?)
        }
    };

    let mut buffered_reader = io::BufReader::new(reader);

    let vault_dir = use_or_create_dir(&config.base, matches.value_of("vault").unwrap_or(""))?;

    let output_dir = use_or_create_dir(&vault_dir, matches.value_of("output").unwrap_or(""))?;

    let splitter = Split::new(output_dir, 512);

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

    io::copy(&mut buffered_reader, &mut writer)?;

    Ok(())
}

fn use_or_create_dir(base: &Path, dir: &str) -> io::Result<PathBuf> {
    if dir.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Empty path given",
        ));
    }

    let dir_path = Path::new(dir);
    let components: Vec<Component> = dir_path.components().collect();

    if components.len() != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Invalid path {} given", dir),
        ));
    }

    let base_dir_path = match components.first() {
        Some(Component::Normal(_)) => base.join(dir_path),
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Invalid path {} given", dir),
            ));
        }
    };

    if let Err(err) = fs::read_dir(&base_dir_path) {
        if err.kind() == io::ErrorKind::NotFound {
            log::info!("Creating directory {:?}", base_dir_path);
            fs::create_dir(&base_dir_path)?;
        } else {
            // PermissionDenied or NotADirectory
            log::error!("Cannot use path {:?}", base_dir_path);
            return Err(err);
        }
    } else {
        log::trace!("Using directory {:?}", base_dir_path);
    }

    Ok(base_dir_path)
}
