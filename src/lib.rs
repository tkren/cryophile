mod aws;
mod backup;
pub mod cli;
pub mod config;
pub mod encoder;
mod freeze;
mod openpgp;
mod recipient;
mod restore;
mod split;
mod thaw;

use cli::error::CliError;
use cli::CliResult;
use cli::Command;
pub use config::Config;
pub use encoder::FinalEncoder;
use log::log_enabled;
pub use split::Split;
use std::env;
use std::fs;
use std::io;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};

fn use_dir_atomic_create_maybe(
    dir_path: &Path,
    create_dir: Option<bool>,
    recursive: Option<bool>,
) -> io::Result<()> {
    if create_dir.unwrap_or(false) {
        log::info!("Creating directory {dir_path:?}");
        // first mkdir the parent path, ignoring if it exists, and then perfrom
        // atomic creation of the final element in dir_path
        // https://rcrowley.org/2010/01/06/things-unix-can-do-atomically.html
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o755);

        builder.recursive(recursive.unwrap_or(false));
        if let Some(parent) = dir_path.parent() {
            builder.create(parent).map_err(|err| {
                io::Error::new(
                    err.kind(),
                    format!("Cannot create {path:?}: {err}", path = parent.display()),
                )
            })?;
        }

        // force failure if full dir_path already exists
        builder.recursive(false);
        builder.create(dir_path).map_err(|err| {
            io::Error::new(
                err.kind(),
                format!("Cannot create {path:?}: {err}", path = dir_path.display()),
            )
        })?;
    } else if let Err(err) = fs::read_dir(dir_path) {
        // PermissionDenied, NotADirectory, NotFound, etc.
        log::error!("Cannot use directory {dir_path:?}");
        return Err(err);
    }

    Ok(())
}

fn use_base_dir(base: &xdg::BaseDirectories) -> io::Result<PathBuf> {
    let state_home = base.get_state_home();
    match fs::metadata(&state_home) {
        Err(_err) => {
            log::info!("Creating state directory {state_home:?}");
            match base.create_state_directory("") {
                Ok(state_path) => Ok(state_path),
                Err(err) => Err(err),
            }
        }
        Ok(metadata) => {
            if !metadata.is_dir() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Base state home {state_home:?} is not an existing directory"),
                ));
            }
            Ok(state_home)
        }
    }
}

pub fn base_directory_profile(_subcommand: &str) -> Result<xdg::BaseDirectories, CliError> {
    match xdg::BaseDirectories::with_prefix(clap::crate_name!()) {
        Ok(base_dirs) => Ok(base_dirs),
        Err(err) => Err(CliError::BaseDirError(err, CliResult::ConfigError)),
    }
}

pub fn setup(debug: usize, quiet: bool) -> Result<(), CliError> {
    // setup logger using environment
    let env = env_logger::Env::new()
        .filter("PERMAFRUST_LOG")
        .write_style("PERMAFRUST_LOG_STYLE");

    env_logger::try_init_from_env(env)?;

    match debug {
        1 if !log_enabled!(log::Level::Debug) => {
            log::set_max_level(log::LevelFilter::Debug);
        }
        (2..) if !log_enabled!(log::Level::Trace) => {
            log::set_max_level(log::LevelFilter::Trace);
        }
        _ => { /* 1 and debug-enabled or 0, 2.. and trace-enabled: noop */ }
    }

    // prioritize quiet
    if quiet && log_enabled!(log::Level::Warn) {
        log::set_max_level(log::LevelFilter::Error);
    }

    Ok(())
}

pub fn log_versions() {
    log::debug!(
        "aws_sdk_s3 version {version:?}",
        version = aws_sdk_s3::PKG_VERSION
    );
    log::debug!(
        "aws_types version {version:?}",
        version = aws_types::build_metadata::BUILD_METADATA.core_pkg_version
    );
    log::debug!(
        "sequoia_openpgp version {version:?}",
        version = sequoia_openpgp::VERSION
    );
}

pub fn run(config: &Config) -> Result<(), CliError> {
    // setup base directory
    let base_pathbuf: PathBuf = use_base_dir(&config.base)?;
    log::trace!("Using base state directory {base_pathbuf:?}");

    let spool = &config.cli.spool;
    use_dir_atomic_create_maybe(spool, None, None)?;
    log::trace!("Using spool directory {spool:?}");

    // perform requested command
    match &config.cli.command {
        Command::Backup(backup) => backup::perform_backup(&config.cli, backup)?,
        Command::Freeze(freeze) => freeze::perform_freeze(&config.cli, freeze, &config.base)?,
        Command::Restore(restore) => restore::perform_restore(&config.cli, restore)?,
        Command::Thaw(thaw) => thaw::perform_thaw(&config.cli, thaw)?,
    };
    Ok(())
}
