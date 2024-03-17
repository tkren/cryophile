// Copyright The Cryophile Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use std::io::{self, Read};

use super::CompressionType;

pub struct Decompressor<'a> {
    input: Box<dyn 'a + io::Read>,
    compression: Option<CompressionType>,
}

impl<'a> Decompressor<'a> {
    pub fn new<R: 'a + io::Read>(input: R) -> Self {
        Self {
            input: Box::new(input),
            compression: None,
        }
    }

    pub fn with_compression(self, compression: CompressionType) -> Self {
        Self {
            input: self.input,
            compression: Some(compression),
        }
    }

    fn magic_decompressor<W: io::Write + ?Sized>(mut self, writer: &mut W) -> io::Result<u64> {
        // read 4 byte magic header and guess compression algorithm
        let mut magic = [0u8; 4];
        let mut buf: &mut [u8] = &mut magic;
        let mut bytes_read = 0usize;

        while !buf.is_empty() {
            match self.input.read(buf) {
                Ok(0) => break,
                Ok(n) => {
                    let tmp = buf;
                    buf = &mut tmp[n..];
                    bytes_read += n;
                }
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }

        if !buf.is_empty() {
            assert!(bytes_read < 4);
            // could not read full magic header, just dump what we have read to output
            let mut input: &[u8] = &magic[..bytes_read];
            return io::copy(&mut input, writer);
        }

        assert!(bytes_read == 4);
        let magic_input: &[u8] = &magic[..];
        let input = magic_input.chain(self.input);
        let mut decompressor: Box<dyn io::Read> = match u32::from_le_bytes(magic) {
            0xFD2FB528 => {
                // zstd magic: https://github.com/facebook/zstd/blob/dev/doc/zstd_compression_format.md
                log::info!("Using Zstandard decompression…");
                Box::new(zstd::stream::Decoder::new(input)?)
            }
            0x184D2204 => {
                // lz4 magic: https://github.com/lz4/lz4/blob/dev/doc/lz4_Frame_format.md
                log::info!("Using LZ4 decompression…");
                Box::new(lz4_flex::frame::FrameDecoder::new(input))
            }
            _ => {
                log::info!("Using no decompression…");
                Box::new(input)
            }
        };
        io::copy(&mut decompressor, writer)
    }

    pub fn copy_to<W: io::Write + ?Sized>(self, writer: &mut W) -> io::Result<u64> {
        if let Some(compression_type) = self.compression {
            let mut decompressor = match compression_type {
                CompressionType::None => {
                    log::info!("Using no decompression…");
                    self.input
                }
                CompressionType::Lz4 => {
                    log::info!("Using LZ4 decompression…");
                    Box::new(lz4_flex::frame::FrameDecoder::new(self.input))
                }
                CompressionType::Zstd => {
                    log::info!("Using Zstandard decompression…");
                    Box::new(zstd::stream::Decoder::new(self.input)?)
                }
            };
            io::copy(&mut decompressor, writer)
        } else {
            self.magic_decompressor(writer)
        }
    }
}
