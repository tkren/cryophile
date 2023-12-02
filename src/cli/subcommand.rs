// Copyright The Permafrust Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use super::constants::DEFAULT_CHUNK_SIZE;
use super::parse::{parse_chunk_size, parse_fd, parse_keyring, parse_uuid};

#[cfg(feature = "age")]
use super::parse::parse_recipient;
#[cfg(feature = "age")]
use crate::crypto::age::RecipientSpec;

use crate::compression::CompressionType;
use clap::{value_parser, Parser, Subcommand};
use sequoia_openpgp::Cert;
use std::fmt;
use std::path::PathBuf;

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
    #[arg(short = 'C', long, help = "compression type", value_enum, default_value_t = CompressionType::default())]
    pub compression: CompressionType,

    #[arg(short, long, help = "input file", value_parser = value_parser!(PathBuf))]
    pub input: Option<PathBuf>,

    #[arg(short, long, help = "keyring", action = clap::ArgAction::Append, required = true, value_parser = parse_keyring)]
    pub keyring: Vec<Vec<Cert>>,

    #[arg(short, long, help = "prefix path in vault", value_parser = value_parser!(PathBuf))]
    pub prefix: Option<PathBuf>,

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

    #[arg(short, long, help = "prefix path in vault", value_parser = value_parser!(PathBuf))]
    pub prefix: Option<PathBuf>,

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
    #[arg(short, long, help = "keyring", action = clap::ArgAction::Append, required = true, value_parser = parse_keyring)]
    pub keyring: Vec<Vec<Cert>>,

    #[arg(short = 'P', long, help = "read password from file descriptor", value_parser = parse_fd)]
    pub pass_fd: Option<i32>,

    #[arg(short, long, help = "output file", value_parser = value_parser!(PathBuf))]
    pub output: Option<PathBuf>,

    #[arg(short, long, help = "prefix path in vault", value_parser = value_parser!(PathBuf))]
    pub prefix: Option<PathBuf>,

    #[arg(short, long, help = "vault", value_parser = parse_uuid)]
    pub vault: uuid::Uuid,
}
