use clap::ValueEnum;

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum CompressionType {
    #[default]
    None,
    Lz4,
    Zstd,
}
