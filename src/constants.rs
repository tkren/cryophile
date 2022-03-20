pub static DEFAULT_CHUNK_SIZE: &str = "512";

pub static DEFAULT_SPOOL_PATH: &str = "/var/spool/permafrust";

#[derive(Clone, Copy)]
pub enum CompressionType {
    None,
    Lz4,
    Zstd,
}

impl From<CompressionType> for &'static str {
    fn from(compression_type: CompressionType) -> Self {
        match compression_type {
            CompressionType::None => "none",
            CompressionType::Lz4 => "lz4",
            CompressionType::Zstd => "zstd",
        }
    }
}

pub static DEFAULT_COMPRESSION: CompressionType = CompressionType::None;
