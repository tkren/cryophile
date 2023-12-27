// Copyright The Permafrust Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

pub mod constants;
pub mod error;
pub mod parse;
pub mod result;
mod subcommand;

use clap::Parser;
use std::path::PathBuf;

pub use self::constants::{
    DEFAULT_CHUNK_SIZE, DEFAULT_CONFIG_PATH, DEFAULT_SPOOL_PATH, UNSAFE_PREFIX,
};
pub use self::error::CliError;
use self::parse::{parse_config, parse_spool};
pub use self::result::CliResult;
pub use self::subcommand::{Backup, Command, Freeze, Restore, Thaw};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = clap::crate_description!())]
#[command(propagate_version = true)]
#[command(subcommand_required = true)]
#[command(arg_required_else_help = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Spool directory containing all backup and restore queues
    #[arg(
        short = 'S', long, value_parser = parse_spool,
        default_value_os_t = PathBuf::from(DEFAULT_SPOOL_PATH),
        value_name = "DIRECTORY",
        help = "Spool directory containing all backup and restore queues",
    )]
    pub spool: PathBuf,

    /// Configuration file
    #[arg(
        short = 'c', long, value_parser = parse_config,
        default_value_os_t = PathBuf::from(DEFAULT_CONFIG_PATH),
        value_name = "FILE",
        help = "Configuration file",
    )]
    pub config: PathBuf,

    /// Print debug information verbosely
    #[arg(
        short,
        long,
        action = clap::ArgAction::Count,
        help = "Print debug information verbosely"
    )]
    pub debug: u8,

    /// Quiet mode
    #[arg(short, long, help = "Quiet mode")]
    pub quiet: bool,
}
