// Copyright The Cryophile Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use std::io;

use crate::core::Split;

pub trait CompleteEncoder: io::Write {
    fn complete(&mut self) -> io::Result<()> {
        log::trace!("Complete encoder");
        Ok(())
    }
}

impl<W: io::Write> CompleteEncoder for lz4_flex::frame::FrameEncoder<W> {
    fn complete(&mut self) -> io::Result<()> {
        log::trace!("Complete LZ4 encoder");
        if let Err(err) = self.try_finish() {
            log::error!("Cannot finish LZ4 encoder: {err:?}");
        }
        Ok(())
    }
}

impl<W: io::Write> CompleteEncoder for zstd::stream::Encoder<'_, W> {
    fn complete(&mut self) -> io::Result<()> {
        log::trace!("Complete ZStd encoder");
        if let Err(err) = self.do_finish() {
            log::error!("Cannot finish ZStd encoder: {err:?}");
            return Err(err);
        }
        Ok(())
    }
}

impl CompleteEncoder for Split {}

pub struct FinalEncoder {
    encoder: Box<dyn CompleteEncoder>,
}

impl FinalEncoder {
    pub fn new(enc: Box<dyn CompleteEncoder>) -> Self {
        FinalEncoder { encoder: enc }
    }
}

impl io::Write for FinalEncoder {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.encoder.write(buf)
    }

    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
        self.encoder.write_vectored(bufs)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.encoder.flush()
    }
}

impl Drop for FinalEncoder {
    fn drop(&mut self) {
        log::trace!("Dropping FinalEncoder");
        if let Err(err) = self.encoder.complete() {
            log::error!("Cannot complete FinalEncoder: {err}");
        }
    }
}
