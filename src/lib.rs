pub mod cli;
pub mod command;
pub mod compression;
pub mod config;
pub mod core;
mod crypto;

use cli::error::CliError;
use cli::CliResult;
use cli::Command;
pub use config::Config;
use log::log_enabled;
use std::env;
use std::path::PathBuf;

use crate::command::backup;
use crate::command::freeze;
use crate::command::restore;
use crate::command::thaw;
use crate::core::path::CreateDirectory;

pub fn base_directory_profile(_subcommand: &Command) -> Result<xdg::BaseDirectories, CliError> {
    match xdg::BaseDirectories::with_prefix(clap::crate_name!()) {
        Ok(base_dirs) => Ok(base_dirs),
        Err(err) => Err(CliError::BaseDirError(err, CliResult::ConfigError)),
    }
}

pub fn setup(debug: u8, quiet: bool) -> Result<(), CliError> {
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
    let base_pathbuf: PathBuf = core::path::use_base_dir(&config.base)?;
    log::trace!("Using base state directory {base_pathbuf:?}");

    let spool = &config.cli.spool;
    core::path::use_dir_atomic_create_maybe(spool, CreateDirectory::No)?;
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
