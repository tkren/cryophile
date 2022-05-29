use std::fmt;
use std::str::FromStr;
use thiserror::Error;

pub static DEFAULT_CHUNK_SIZE: usize = 512;

pub static DEFAULT_SPOOL_PATH: &str = "/var/spool/permafrust";

#[derive(Clone, Copy, Debug)]
pub enum CompressionType {
    None,
    Lz4,
    Zstd,
}

impl Default for CompressionType {
    fn default() -> Self {
        CompressionType::None
    }
}

impl From<CompressionType> for String {
    fn from(compression_type: CompressionType) -> Self {
        String::from(match compression_type {
            CompressionType::None => "none",
            CompressionType::Lz4 => "lz4",
            CompressionType::Zstd => "zstd",
        })
    }
}

impl fmt::Display for CompressionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", String::from(*self))
    }
}

#[derive(Error, Debug)]
pub enum ParseCompressionError {
    #[error("unknown compression type")]
    Unknown,
}

impl FromStr for CompressionType {
    type Err = ParseCompressionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let compression = match s {
            "none" => CompressionType::None,
            "lz4" => CompressionType::Lz4,
            "zstd" => CompressionType::Zstd,
            _ => return Err(ParseCompressionError::Unknown),
        };
        Ok(compression)
    }
}
