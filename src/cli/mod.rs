pub mod constants;
pub mod error;
pub mod parse;
pub mod result;
mod subcommand;

use clap::{value_parser, Parser};
use std::path::PathBuf;

pub use self::constants::{DEFAULT_CHUNK_SIZE, DEFAULT_SPOOL_PATH};
pub use self::error::CliError;
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

    /// Spool directory containing all backup and restore state
    #[arg(
        short, long, value_parser = value_parser!(PathBuf),
        default_value_os_t = PathBuf::from(DEFAULT_SPOOL_PATH),
        value_name = "FILE",
        help = "Spool directory containing all backup and restore state",
    )]
    pub spool: PathBuf,

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
