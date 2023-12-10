// Copyright The Permafrust Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use std::{fmt, fs, io, path::PathBuf};

use crossbeam::channel::{Receiver, Sender};

use super::channel::channel_recv_error;

pub struct Cat {
    tx: Sender<Option<PathBuf>>,
    rx: Receiver<Option<PathBuf>>,
    pos: usize,             // written bytes of current file
    tot: usize,             // total bytes written
    num: u64,               // number of files concatenated
    file: Option<fs::File>, // current input file
    mark_failed: bool,      // Cat had an error
}

impl fmt::Debug for Cat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Cat {{ total_bytes: {total_bytes}, chunks: {chunks}, mark_failed: {mark_failed}, file: {file:?}}}",
            total_bytes = self.tot,
            chunks = self.num,
            mark_failed = self.mark_failed,
            file = self.file
        )
    }
}

impl Cat {
    pub fn new() -> Self {
        let (tx, rx) = crossbeam::channel::unbounded();
        Self {
            tx,
            rx,
            pos: 0,
            num: 0,
            tot: 0,
            file: None,
            mark_failed: false,
        }
    }

    pub fn tx(&self) -> Sender<Option<PathBuf>> {
        self.tx.to_owned()
    }

    fn ok_or_retry(&mut self, n: usize) -> io::Result<usize> {
        if n == 0 {
            // reached eof most likely, wait for new path
            self.file = None;
            self.pos = 0;
            return Err(io::Error::new(io::ErrorKind::Interrupted, "Retry"));
        }
        self.pos += n;
        self.tot += n;
        Ok(n)
    }

    pub fn clear(&mut self) {
        let (tx, rx) = crossbeam::channel::unbounded();
        self.tx = tx;
        self.rx = rx;
        self.pos = 0;
        self.tot = 0;
        self.num = 0;
        self.file = None;
        self.mark_failed = false;
    }
}

impl Default for Cat {
    fn default() -> Self {
        Self::new()
    }
}

impl io::Read for Cat {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(mut file) = self.file.as_ref() {
            let n = file.read(buf)?;
            self.ok_or_retry(n)
        } else if let Some(path) = self.rx.recv().map_err(channel_recv_error)? {
            loop {
                let mut file = match fs::File::options().read(true).open(&path) {
                    Ok(file) => file,
                    Err(err) if err.kind() == io::ErrorKind::Interrupted => {
                        // io::ErrorKind::Interrupted we must retry actually
                        log::debug!("Retrying interrupted open for {path:?}: {err}");
                        continue;
                    }
                    Err(err) => {
                        log::warn!("Ignoring that we could not open {path:?}: {err}");
                        return self.ok_or_retry(0);
                    }
                };
                self.num += 1;
                break file.read(buf).and_then(|n| {
                    self.file = Some(file);
                    self.ok_or_retry(n)
                });
            }
        } else {
            // self.file is None and received None from channel, just shutdown
            self.clear();
            Ok(0)
        }
    }
}
