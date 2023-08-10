// Copyright The Permafrust Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use std::fs;
use std::io;
use std::io::Write;
use std::os::unix::prelude::OpenOptionsExt;
use std::path::Path;
use std::path::PathBuf;

use crate::core::constants::CHUNK_FILE_MODE;

pub struct Split {
    num: usize,             // maximum size of each split
    pos: usize,             // written bytes of current split
    tot: u64,               // total bytes written
    val: u64,               // number of file splits
    prefix: PathBuf,        // path prefix
    file: Option<fs::File>, // current output file
    mark_failed: bool,      // Split had an error
}

impl Drop for Split {
    fn drop(&mut self) {
        log::trace!(
            "Split statistics: prefix={prefix:?} total_bytes={total_bytes} chunks={chunks} failed={failed}",
            prefix=self.prefix,
            total_bytes=self.tot,
            chunks=self.val,
            failed=self.mark_failed
        );
        if let Err(err) = self.flush() {
            let path_buf = self.current_path_buf();
            log::error!(
                "Cannot flush {path}: {err}",
                path = path_buf.as_path().display()
            );
        }
    }
}

impl Split {
    pub fn new(prefix_path: &Path, chunk_prefix: &str, num: usize) -> Self {
        Split {
            num,
            pos: 0,
            tot: 0,
            val: 0,
            prefix: prefix_path.join(chunk_prefix),
            file: None,
            mark_failed: false,
        }
    }

    pub fn clear(&mut self) -> io::Result<()> {
        let result = self.flush();
        self.pos = 0;
        self.tot = 0;
        self.val = 0;
        self.file = None;
        self.mark_failed = false;
        result
    }

    pub fn written(&self) -> u64 {
        self.tot
    }

    fn current_path_buf(&mut self) -> PathBuf {
        self.prefix.with_extension(self.val.to_string())
    }

    fn use_file_or_next(&mut self) -> io::Result<usize> {
        assert!(self.pos <= self.num, "file position exceeded max size");

        if self.mark_failed {
            log::error!(
                "Split is marked failed at {total_bytes}",
                total_bytes = self.tot
            );
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "Split is marked failed at {total_bytes}",
                    total_bytes = self.tot
                ),
            ));
        }

        if self.file.is_some() && self.pos < self.num {
            return Ok(self.num - self.pos);
        }

        self.val += 1;
        let path_buf = self.current_path_buf();
        let file_path = path_buf.as_path();

        log::trace!("Creating new chunk {file_path:?}");

        self.file = match fs::File::options()
            .write(true)
            .create_new(true)
            .mode(CHUNK_FILE_MODE)
            .open(file_path)
        {
            Ok(file) => Some(file),
            Err(err) => {
                self.mark_failed = true;

                log::error!(
                    "Cannot create new file {path}, marking Split failed",
                    path = file_path.display()
                );

                return Err(io::Error::new(
                    err.kind(),
                    format!("Cannot create new file {path}", path = file_path.display()),
                ));
            }
        };
        self.pos = 0;

        Ok(self.num)
    }

    fn write_once(&mut self, buf: &[u8]) -> io::Result<usize> {
        let buf_len = buf.len();
        if buf_len == 0 {
            return Ok(0);
        }
        assert!(buf_len <= self.num, "buffer too large");

        if self.mark_failed {
            log::error!(
                "Split failed at position {total_bytes}, ignoring write request",
                total_bytes = self.tot
            );
            return Ok(0);
        }

        let _remaining_split = self.use_file_or_next()?;

        let mut file = self.file.as_ref().unwrap();

        let mut slice = buf;
        let n = io::copy(&mut slice, &mut file)?;

        let offset = usize::try_from(n).expect("copied buffer exceeds usize");

        self.tot += n;
        self.pos += offset;
        assert!(self.pos <= self.num, "Split.pos > Split.num");

        Ok(offset)
    }
}

impl io::Write for Split {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.mark_failed {
            log::error!(
                "Ignoring error at position {total_bytes}",
                total_bytes = self.tot
            );
            return Ok(0);
        }

        let buf_len = buf.len();
        let mut written = 0;

        let remainder = self.num - self.pos;

        let (head, tail) = if remainder < buf_len {
            // buf_len > remainder: split buf into buf[0..remainder-1], buf[remainder..]
            buf.split_at(remainder)
        } else {
            // buf_len <= remainder: split buf into buf[0..buf_len-1] (buf), buf[buf_len..] ([])
            buf.split_at(buf_len)
        };

        log::trace!(
            "Head remaining={remaining:?} prefix={prefix:?} total_bytes={total_bytes} chunks={chunks}",
            remaining=remainder,
            prefix=self.prefix,
            total_bytes=self.tot,
            chunks=self.val
        );

        // write left slice of length remainder or buf_len
        written += self.write_once(head)?;

        if !tail.is_empty() {
            log::trace!(
                "Tail remaining={remaining:?} prefix={prefix:?} total_bytes={total_bytes} chunks={chunks}",
                remaining = buf_len.saturating_sub(remainder),
                prefix=self.prefix,
                total_bytes=self.tot,
                chunks=self.val
            );

            // write right slice in chunks of length self.num (last chunk at most self.num)
            for chunk in tail.chunks(self.num) {
                written += self.write_once(chunk)?;
            }
        }

        assert_eq!(buf_len, written, "buf.len() != written");

        Ok(written)
    }

    #[inline]
    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
        let total_len = bufs.iter().map(|buf| self.write(buf).unwrap()).sum();
        Ok(total_len)
    }

    // #[inline]
    // fn is_write_vectored(&self) -> bool {
    //     true
    // }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        let Some(file) = &mut self.file else {
            log::trace!("Nothing to flush…");
            return Ok(())
        };
        log::trace!(
            "Attempting flush: prefix={prefix:?} total_bytes={total_bytes} chunks={chunks}",
            prefix = self.prefix,
            total_bytes = self.tot,
            chunks = self.val
        );
        file.flush()
    }
}
