use clap::ValueEnum;

#[derive(Clone, Copy, Debug, ValueEnum)]
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
