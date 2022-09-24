pub mod constants;
pub mod error;
pub mod result;

use crate::compression::CompressionType;
use crate::crypto::age::RecipientSpec;
use crate::crypto::openpgp::openpgp_error;
use clap::{value_parser, Parser, Subcommand};
use sequoia_openpgp::cert::CertParser;
use sequoia_openpgp::parse::Parse;
use sequoia_openpgp::Cert;
use std::collections::VecDeque;
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

pub use self::constants::{DEFAULT_CHUNK_SIZE, DEFAULT_SPOOL_PATH};
pub use self::error::CliError;
pub use self::result::CliResult;

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

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Queue input for upload
    #[command(arg_required_else_help = false)]
    Backup(Backup),
    /// Upload backup
    #[command(arg_required_else_help = false)]
    Freeze(Freeze),
    /// Download backup
    #[command(arg_required_else_help = false)]
    Thaw(Thaw),
    /// Uncompress and decrypt downloaded backup files
    #[command(arg_required_else_help = false)]
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
#[command(about = "Not shown")]
pub struct Backup {
    #[arg(short = 'C', long, help = "compression type", value_parser = parse_compression, default_value_t = CompressionType::default())]
    pub compression: CompressionType,

    #[arg(short, long, help = "input file", value_parser = value_parser!(PathBuf))]
    pub input: Option<PathBuf>,

    #[arg(short, long, help = "keyring", required = true, value_parser = parse_keyring)]
    pub keyring: VecDeque<Cert>,

    #[arg(short, long, help = "output file", value_parser = value_parser!(PathBuf))]
    pub output: Option<PathBuf>,

    #[arg(short, long, help = "recipient", value_parser = parse_recipient)]
    pub recipient: Option<Vec<RecipientSpec>>,

    #[arg(short, long, help = "chunk size", value_parser = parse_chunk_size, default_value_t = DEFAULT_CHUNK_SIZE)]
    pub size: usize,

    #[arg(short, long, help = "vault", value_parser = parse_uuid)]
    pub vault: uuid::Uuid,
}

#[derive(Parser, Debug)]
#[command(about = "Not shown")]
pub struct Freeze {
    #[arg(short, long, help = "config file", value_parser = value_parser!(PathBuf))]
    pub config: Option<PathBuf>,
}

#[derive(Parser, Debug)]
#[command(about = "Not shown")]
pub struct Thaw {}

#[derive(Parser, Debug)]
#[command(about = "Not shown")]
pub struct Restore {
    #[arg(short, long, help = "output file", value_parser = value_parser!(PathBuf))]
    pub output: Option<PathBuf>,
}

fn parse_compression(s: &str) -> Result<CompressionType, String> {
    let compression =
        CompressionType::from_str(s).map_err(|e| format!("Cannot parse compression type: {e}"))?;
    Ok(compression)
}

fn parse_chunk_size(s: &str) -> Result<usize, String> {
    let parse_config = parse_size::Config::new()
        .with_binary()
        .with_byte_suffix(parse_size::ByteSuffix::Deny);
    let parse_size_result = parse_config
        .parse_size(s)
        .map_err(|e| format!("Cannot parse chunk size: {e}"))?;
    let chunk_size = usize::try_from(parse_size_result)
        .map_err(|e| format!("Cannot parse chunk size (size exceeds usize): {e}"))?;
    Ok(chunk_size)
}

fn parse_uuid(s: &str) -> Result<uuid::Uuid, String> {
    let uuid = uuid::Uuid::parse_str(s).map_err(|e| format!("Cannot parse uuid: {e}"))?;
    Ok(uuid)
}

fn parse_recipient(s: &str) -> Result<RecipientSpec, String> {
    let recipient = s
        .parse::<RecipientSpec>()
        .map_err(|e| format!("Cannot parse age: {e}"))?;
    Ok(recipient)
}

fn parse_keyring(s: &str) -> Result<VecDeque<Cert>, String> {
    let mut cert_list: VecDeque<Cert> = VecDeque::new();
    let parser = CertParser::from_file(s).map_err(|e| openpgp_error(e).to_string())?;
    for parsed_cert in parser {
        if let Err(err) = parsed_cert {
            return Err(openpgp_error(err).to_string());
        }
        let result: Cert = parsed_cert.unwrap();
        cert_list.push_back(result);
    }
    if cert_list.is_empty() {
        return Err(format!("Keyring {s} is empty"));
    }
    Ok(cert_list)
}
