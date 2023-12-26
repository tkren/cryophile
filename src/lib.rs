// Copyright The Permafrust Authors.
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

use cli::error::CliError;
use cli::CliResult;
use cli::Command;
pub use config::Config;
use env_logger::Builder;
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
    // setup logger using environment:
    // prioritize command-line args over environment variables, and quiet over debug
    let env = env_logger::Env::new().write_style("PERMAFRUST_LOG_STYLE");
    let env = if quiet {
        env.filter_or("", "error")
    } else {
        match debug {
            1 => env.filter_or("", "debug"),
            (2..) => env.filter_or("", "trace"),
            _ => env.filter_or("PERMAFRUST_LOG", "info"),
        }
    };
    Builder::new().parse_env(env).try_init()?;
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
