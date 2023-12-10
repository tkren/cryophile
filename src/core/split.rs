// Copyright The Permafrust Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use std::io::Write;
use std::os::fd::AsFd;
use std::os::fd::AsRawFd;
use std::os::unix::prelude::OpenOptionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::{fmt, fs, io};

use nix::fcntl::FallocateFlags;

use crate::core::constants::CHUNK_FILE_MODE;

fn errno_error(e: nix::errno::Errno) -> io::Error {
    io::Error::from_raw_os_error(e as i32)
}

fn log_io_error(err: io::Error, error: String) -> io::Error {
    log::error!("{error} ({err})");
    io::Error::new(err.kind(), error)
}

pub struct Split {
    num: usize,             // maximum size of each split
    pos: usize,             // written bytes of current split
    tot: u64,               // total bytes written
    val: u64,               // number of file splits
    incoming: PathBuf,      // incoming chunk prefix
    outgoing: PathBuf,      // outgoing link prefix
    file: Option<fs::File>, // current output file
    mark_failed: bool,      // Split had an error
}

impl fmt::Debug for Split {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Split {{ prefix: {prefix:?}, total_bytes: {total_bytes}, chunks: {chunks}, mark_failed: {mark_failed}, file: {file:?}}}",
            prefix = self.incoming,
            total_bytes = self.tot,
            chunks = self.val,
            mark_failed = self.mark_failed,
            file = self.file
        )
    }
}

impl Drop for Split {
    #[tracing::instrument(level = "trace")]
    fn drop(&mut self) {
        // flush data
        if let Err(err) = self.flush() {
            log::error!("Cannot flush: {err}");
            return;
        }
        // truncate and link current incoming chunk outgoing
        if let Err(err) = self.outgoing_chunk() {
            log::error!("Cannot truncate and link: {err}");
        }
    }
}

impl Split {
    pub fn new(incoming: &Path, outgoing: &Path, chunk_prefix: &str, num: usize) -> Self {
        Split {
            num,
            pos: 0,
            tot: 0,
            val: 0,
            incoming: incoming.join(chunk_prefix),
            outgoing: outgoing.join(chunk_prefix),
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

    fn current_incoming_path(&self) -> PathBuf {
        self.incoming.with_extension(self.val.to_string())
    }

    fn current_outgoing_path(&self) -> PathBuf {
        self.outgoing.with_extension(self.val.to_string())
    }

    #[tracing::instrument(level = "trace")]
    fn outgoing_chunk(&mut self) -> io::Result<()> {
        // link current incoming chunk outgoing
        let Some(file) = self.file.as_ref() else {
            return Ok(());
        };
        let incoming = self.current_incoming_path();
        let outgoing = self.current_outgoing_path();
        file.sync_data().map_err(|err| {
            self.mark_failed = true;
            log_io_error(err, format!("Cannot sync incoming {incoming:?}"))
        })?;

        // truncate fallocate'd file to actual bytes written
        if self.pos < self.num {
            tracing::event!(
                name: "ftruncate",
                tracing::Level::TRACE,
                action = "truncate",
                incoming = format!("{incoming:?}", incoming = incoming),
                len = self.pos
            );
            let len = i64::try_from(self.pos).expect("chunk position exceeds usize");
            nix::unistd::ftruncate(file.as_fd(), len)
                .map_err(errno_error)
                .map_err(|err| {
                    self.mark_failed = true;
                    log_io_error(err, format!("Cannot ftruncate incoming {incoming:?}"))
                })?;
        }

        tracing::event!(
            name: "hard_link",
            tracing::Level::TRACE,
            action = "link",
            incoming = format!("{incoming:?}", incoming = incoming),
            outgoing = format!("{outgoing:?}", outgoing = outgoing)
        );
        fs::hard_link(&incoming, &outgoing).map_err(|err| {
            self.mark_failed = true;
            log_io_error(err, format!("Cannot create new outgoing {outgoing:?}"))
        })?;
        tracing::event!(
            name: "remove_file",
            tracing::Level::TRACE,
            action = "unlink",
            incoming = format!("{incoming:?}", incoming = incoming)
        );
        fs::remove_file(incoming).map_err(|err| {
            self.mark_failed = true;
            log_io_error(err, format!("Cannot unlink incoming {outgoing:?}"))
        })?;
        Ok(())
    }

    #[tracing::instrument(level = "trace")]
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

        // use file
        if self.file.is_some() && self.pos < self.num {
            return Ok(self.num - self.pos);
        }

        // link current incoming chunk outgoing
        self.outgoing_chunk()?;

        // open next chunk
        self.val += 1;
        let incoming = self.current_incoming_path();

        tracing::event!(
            name: "open",
            tracing::Level::TRACE,
            action = "create",
            incoming = format!("{incoming:?}", incoming = incoming),
            len = self.pos
        );

        let file = fs::File::options()
            .write(true)
            .create_new(true)
            .mode(CHUNK_FILE_MODE)
            .open(&incoming)
            .map_err(|err| {
                self.mark_failed = true;
                log_io_error(err, format!("Cannot create new incoming {incoming:?}"))
            })?;

        self.file = Some(file);
        self.pos = 0;

        let len = i64::try_from(self.num).expect("chunk size exceeds usize");
        tracing::event!(
            name: "fallocate",
            tracing::Level::TRACE,
            action = "fallocate",
            incoming = format!("{incoming:?}", incoming = incoming),
            len = len
        );

        if let Err(err) = nix::fcntl::fallocate(
            self.file.as_ref().unwrap().as_raw_fd(),
            FallocateFlags::empty(),
            0,
            len,
        )
        .map_err(errno_error)
        {
            log::warn!("Need more disk space to fallocate {len} bytes for new fragment {incoming:?} ({err}), retrying.");
            self.file = None;
            fs::remove_file(&incoming).map_err(|err| {
                self.mark_failed = true;
                log_io_error(err, format!("Cannot unlink new fragment {incoming:?}"))
            })?;
            // retry
            return Ok(0);
        };

        Ok(self.num)
    }

    #[tracing::instrument(level = "trace", skip(buf))]
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

        let remaining_bytes = self.use_file_or_next()?;
        if remaining_bytes == 0 {
            return Ok(0);
        }

        // we expect file to be open, use_file_or_next checks this
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
    #[tracing::instrument(level = "trace", skip(buf))]
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

        tracing::event!(
            name: "head",
            tracing::Level::TRACE,
            head_remaining = remainder,
            prefix = format!("{prefix:?}", prefix = self.incoming),
            total_bytes = self.tot,
            chunks = self.val
        );

        // write left slice of length remainder or buf_len
        written += self.write_once(head)?;

        if !tail.is_empty() {
            tracing::event!(
                name: "tail",
                tracing::Level::TRACE,
                tail_remaining = buf_len.saturating_sub(remainder),
                prefix = format!("{prefix:?}", prefix = self.incoming),
                total_bytes = self.tot,
                chunks = self.val
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
    #[tracing::instrument(level = "trace")]
    fn flush(&mut self) -> io::Result<()> {
        let Some(file) = &mut self.file else {
            return Ok(());
        };
        file.flush()
    }
}
