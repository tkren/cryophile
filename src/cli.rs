use crate::constants::{CompressionType, DEFAULT_CHUNK_SIZE, DEFAULT_SPOOL_PATH};
use clap::{Parser, Subcommand};
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = clap::crate_description!())]
#[clap(propagate_version = true)]
#[clap(subcommand_required(true))]
#[clap(arg_required_else_help(true))]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Command,

    /// Turn debugging information on
    #[clap(
        short, long, parse(from_os_str),
        default_value_os_t = PathBuf::from(DEFAULT_SPOOL_PATH),
        value_name = "FILE",
        help = "Base directory containing all backup and restore state",
    )]
    pub base: PathBuf,

    /// Turn debugging information on
    #[clap(
        short,
        long,
        parse(from_occurrences),
        help = "print debug information verbosely"
    )]
    pub debug: usize,

    /// Turn debugging information on
    #[clap(short, long, help = "Quiet mode")]
    pub quiet: bool,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Backup
    #[clap(arg_required_else_help = true)]
    Backup(Backup),
    /// Freeze: upload backup
    #[clap(arg_required_else_help = true)]
    Freeze(Freeze),
    /// Thaw: download backup
    #[clap(arg_required_else_help = true)]
    Thaw(Thaw),
    /// Restore
    #[clap(arg_required_else_help = true)]
    Restore(Restore),
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let command_name = match self {
            Command::Backup(_) => "backup",
            Command::Freeze(_) => "freeze",
            Command::Thaw(_) => "thaw",
            Command::Restore(_) => "restore",
        };
        write!(f, "{command_name}")
    }
}

#[derive(Parser, Debug)]
#[clap(about = "Not shown")]
pub struct Backup {
    #[clap(short, long, help = "compression type", parse(try_from_str=parse_compression), default_value_t = CompressionType::default())]
    pub compression: CompressionType,

    #[clap(short, long, help = "input file", parse(from_os_str))]
    pub input: Option<PathBuf>,

    #[clap(short, long, help = "output file", parse(from_os_str))]
    pub output: Option<PathBuf>,

    #[clap(short, long, help = "chunk size", parse(try_from_str=parse_chunk_size), default_value_t = DEFAULT_CHUNK_SIZE)]
    pub size: usize,

    #[clap(short, long, help = "vault", parse(try_from_str=parse_uuid))]
    pub vault: uuid::Uuid,
}

#[derive(Parser, Debug)]
#[clap(about = "Not shown")]
pub struct Freeze {}

#[derive(Parser, Debug)]
#[clap(about = "Not shown")]
pub struct Thaw {}

#[derive(Parser, Debug)]
#[clap(about = "Not shown")]
pub struct Restore {
    #[clap(short, long, help = "output file", parse(from_os_str))]
    pub output: Option<PathBuf>,
}

fn parse_compression(s: &str) -> Result<CompressionType, String> {
    let compression = CompressionType::from_str(s)
        .map_err(|e| format!("Cannot parse compression type: {msg}", msg = e.to_string()))?;
    Ok(compression)
}

fn parse_chunk_size(s: &str) -> Result<usize, String> {
    let parse_config = parse_size::Config::new()
        .with_binary()
        .with_byte_suffix(parse_size::ByteSuffix::Deny);
    let parse_size_result = parse_config
        .parse_size(s)
        .map_err(|e| format!("Cannot parse chunk size: {msg}", msg = e.to_string()))?;
    let chunk_size = usize::try_from(parse_size_result).map_err(|e| {
        format!(
            "Cannot parse chunk size (size exceeds usize): {msg}",
            msg = e.to_string()
        )
    })?;
    Ok(chunk_size)
}

fn parse_uuid(s: &str) -> Result<uuid::Uuid, String> {
    let uuid = uuid::Uuid::parse_str(s)
        .map_err(|e| format!("Cannot parse uuid: {msg}", msg = e.to_string()))?;
    Ok(uuid)
}