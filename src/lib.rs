// Copyright The Cryophile Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

#![feature(lazy_cell)]

pub mod cli;
pub mod command;
pub mod compression;
pub mod config;
pub mod core;
mod crypto;

use clap::error::ErrorKind;
use cli::error::CliError;
use cli::Cli;
use cli::CliResult;
use cli::Command;
pub use config::Config;
use env_logger::Builder;
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use crate::cli::DEFAULT_CONFIG_PATH;
use crate::command::backup;
use crate::command::freeze;
use crate::command::restore;
use crate::command::thaw;
use crate::config::ConfigFile;
use crate::config::ParseConfigError;

pub fn on_clap_error(err: clap::error::Error) -> Cli {
    err.print().expect("Error writing error");

    let code: CliResult = match err.use_stderr() {
        true => CliResult::Usage,
        false => match err.kind() {
            ErrorKind::DisplayHelp => CliResult::Ok,
            ErrorKind::DisplayVersion => CliResult::Ok,
            ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => CliResult::Usage,
            _ => CliResult::Usage,
        },
    };

    // perform clap::util::safe_exit(code)
    use std::io::Write;

    let _ = std::io::stdout().lock().flush();
    let _ = std::io::stderr().lock().flush();

    std::process::exit(code as i32);
}

pub fn base_directory_profile(_subcommand: &Command) -> Result<xdg::BaseDirectories, CliError> {
    match xdg::BaseDirectories::with_prefix(clap::crate_name!()) {
        Ok(base_dirs) => Ok(base_dirs),
        Err(err) => Err(CliError::BaseDirError(err, CliResult::ConfigError)),
    }
}

pub fn setup(debug: u8, quiet: bool) -> Result<(), CliError> {
    // setup logger using environment:
    // prioritize command-line args over environment variables, and quiet over debug
    let env = env_logger::Env::new().write_style("CRYOPHILE_LOG_STYLE");
    let env = if quiet {
        env.filter_or("", "error")
    } else {
        match debug {
            1 => env.filter_or("", "debug"),
            (2..) => env.filter_or("", "trace"),
            _ => env.filter_or("CRYOPHILE_LOG", "info"),
        }
    };
    if let Err(err) = Builder::new().parse_env(env).try_init() {
        let err: CliError = err.into();
        eprintln!("Cannot initialize cryophile: {err}");
        return Err(err);
    }
    Ok(())
}

pub fn log_versions() {
    log::debug!(
        "aws_sdk_s3 version {version:?}",
        version = aws_sdk_s3::meta::PKG_VERSION
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

pub fn read_config(path: &Path) -> Result<ConfigFile, CliError> {
    match ConfigFile::new(path) {
        Ok(c) => Ok(c),
        Err(err) => match err {
            ParseConfigError::IoError(e) => {
                log::debug!("Cannot read config from {path:?}: {e}");
                let path = PathBuf::from(DEFAULT_CONFIG_PATH);
                Ok(ConfigFile::new(path.as_path()).unwrap_or_default())
            }
            _ => Err(err.into()),
        },
    }
}

pub fn run(cli: Cli) -> Result<CliResult, CliError> {
    log_versions();

    let base_directories = base_directory_profile(&cli.command).unwrap();

    // read config file
    let config_file = if cli.config != PathBuf::from(DEFAULT_CONFIG_PATH) {
        // always fail if --config is given
        ConfigFile::new(cli.config.as_path())?
    } else {
        // do not fail if we cannot read standard config locations, unless there is a config syntax error
        let user_config_path = base_directories.get_config_file("cryophile.toml");
        read_config(&user_config_path)?
    };

    let config = Config::new(base_directories, cli, config_file);

    // setup base directory
    let base_pathbuf: PathBuf = core::path::use_base_dir(&config.base)?;
    log::trace!("Using base state directory {base_pathbuf:?}");

    let spool = &config.cli.spool;
    fs::read_dir(spool)?; // PermissionDenied, NotADirectory, NotFound, etc.
    log::trace!("Using spool directory {spool:?}");

    // perform requested command
    match &config.cli.command {
        Command::Backup(backup) => backup::perform_backup(&config, backup)?,
        Command::Freeze(freeze) => freeze::perform_freeze(&config, freeze)?,
        Command::Restore(restore) => restore::perform_restore(&config, restore)?,
        Command::Thaw(thaw) => thaw::perform_thaw(&config, thaw)?,
    };
    Ok(CliResult::Ok)
}
