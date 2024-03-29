// Copyright The Cryophile Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use super::constants::DEFAULT_CHUNK_SIZE;
use super::parse::{
    parse_chunk_size, parse_fd, parse_keyring, parse_prefix, parse_timestamp_for_ulid, parse_ulid,
    parse_uuid,
};

#[cfg(feature = "age")]
use super::parse::parse_recipient;
#[cfg(feature = "age")]
use crate::crypto::age::RecipientSpec;

use crate::compression::CompressionType;
use clap::{value_parser, Parser, Subcommand};
use sequoia_openpgp::Cert;
use std::fmt;
use std::path::PathBuf;
use ulid::Ulid;

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Compress, encrypt input and queue for upload
    #[command(arg_required_else_help = false)]
    Backup(Backup),
    /// Upload backup
    #[command(arg_required_else_help = false)]
    Freeze(Freeze),
    /// Download backup
    #[command(arg_required_else_help = false)]
    Thaw(Thaw),
    /// Decrypt, uncompress downloaded backup files
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
    #[arg(short = 'C', long, help = "compression type", value_enum, default_value_t = CompressionType::default())]
    pub compression: CompressionType,

    #[arg(short, long, help = "input file", value_parser = value_parser!(PathBuf))]
    pub input: Option<PathBuf>,

    #[arg(short, long, help = "keyring", action = clap::ArgAction::Append, required = true, value_parser = parse_keyring)]
    pub keyring: Vec<Vec<Cert>>,

    #[arg(short, long, help = "prefix path in vault", value_parser = parse_prefix)]
    pub prefix: Option<PathBuf>,

    #[arg(group = "backup-ulid", short, long, help = "backup timestamp", value_parser = parse_timestamp_for_ulid)]
    pub timestamp: Option<Ulid>,

    #[arg(group = "backup-ulid", short, long, help = "backup ulid", value_parser = parse_ulid)]
    pub ulid: Option<Ulid>,

    #[arg(short, long, help = "chunk size", value_parser = parse_chunk_size, default_value_t = DEFAULT_CHUNK_SIZE)]
    pub size: usize,

    #[arg(short, long, help = "vault", value_parser = parse_uuid)]
    pub vault: uuid::Uuid,
}

#[cfg(feature = "age")]
#[derive(Parser, Debug)]
#[command(about = "Not shown")]
pub struct Backup {
    #[arg(short = 'C', long, help = "compression type", value_enum, default_value_t = CompressionType::default())]
    pub compression: CompressionType,

    #[arg(short, long, help = "input file", value_parser = value_parser!(PathBuf))]
    pub input: Option<PathBuf>,

    #[arg(short, long, help = "keyring", action = clap::ArgAction::Append, required = true, value_parser = parse_keyring)]
    pub keyring: Vec<Vec<Cert>>,

    #[arg(short, long, help = "prefix path in vault", value_parser = parse_prefix)]
    pub prefix: Option<PathBuf>,

    #[arg(group = "backup-ulid", short, long, help = "backup timestamp", value_parser = parse_timestamp_for_ulid)]
    pub timestamp: Option<Ulid>,

    #[arg(group = "backup-ulid", short, long, help = "backup ulid", value_parser = parse_ulid)]
    pub ulid: Option<Ulid>,

    #[arg(short, long, help = "recipient", value_parser = parse_recipient)]
    pub recipient: Option<Vec<RecipientSpec>>,

    #[arg(short, long, help = "chunk size", value_parser = parse_chunk_size, default_value_t = DEFAULT_CHUNK_SIZE)]
    pub size: usize,

    #[arg(short, long, help = "vault", value_parser = parse_uuid, requires = "backup-ulid")]
    pub vault: uuid::Uuid,
}

#[derive(Parser, Debug)]
#[command(about = "Not shown")]
pub struct Freeze {
    #[arg(requires = "ulid", short, long, help = "prefix path in vault", value_parser = parse_prefix)]
    pub prefix: Option<PathBuf>,

    #[arg(requires = "vault", short, long, help = "backup ulid", value_parser = parse_ulid)]
    pub ulid: Option<Ulid>,

    #[arg(requires = "prefix", short, long, help = "vault", value_parser = parse_uuid)]
    pub vault: Option<uuid::Uuid>,
}

#[derive(Parser, Debug)]
#[command(about = "Not shown")]
pub struct Thaw {}

#[derive(Parser, Debug)]
#[command(about = "Not shown")]
pub struct Restore {
    #[arg(short = 'C', long, help = "compression type", value_enum)]
    pub compression: Option<CompressionType>,

    #[arg(short, long, help = "keyring", action = clap::ArgAction::Append, required = true, value_parser = parse_keyring)]
    pub keyring: Vec<Vec<Cert>>,

    #[arg(short = 'P', long, help = "read password from file descriptor", value_parser = parse_fd)]
    pub pass_fd: Option<i32>,

    #[arg(short, long, help = "output file", value_parser = value_parser!(PathBuf))]
    pub output: Option<PathBuf>,

    #[arg(short, long, help = "prefix path in vault", value_parser = parse_prefix)]
    pub prefix: Option<PathBuf>,

    #[arg(short, long, help = "vault", value_parser = parse_uuid)]
    pub vault: uuid::Uuid,

    #[arg(short, long, help = "backup ulid", value_parser = parse_ulid)]
    pub ulid: Ulid,
}
