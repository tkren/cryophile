// Copyright The Cryophile Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use std::sync::{mpsc, Mutex};
use std::{fmt, fs, io, path::PathBuf};

use std::sync::mpsc::{Receiver, Sender};

use super::watch::channel_recv_error;

pub struct Cat {
    tx: Sender<Option<PathBuf>>,
    rx: Mutex<Receiver<Option<PathBuf>>>,
    pos: usize,             // written bytes of current file
    tot: usize,             // total bytes written
    num: u64,               // number of files concatenated
    file: Option<fs::File>, // current input file
    mark_failed: bool,      // Cat had an error
    completed: bool,
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
        let (tx, rx) = mpsc::channel();
        Self {
            tx,
            rx: Mutex::new(rx),
            pos: 0,
            num: 0,
            tot: 0,
            file: None,
            mark_failed: false,
            completed: false,
        }
    }

    pub fn tx(&self) -> Sender<Option<PathBuf>> {
        self.tx.to_owned()
    }

    #[tracing::instrument(level = "trace")]
    fn ok_or_retry(&mut self, n: usize) -> io::Result<usize> {
        if n == 0 {
            // reached eof most likely, wait for new path
            tracing::event!(
                tracing::Level::TRACE,
                action = "retry",
                total_bytes = self.tot,
                chunks = self.num
            );
            self.file = None;
            self.pos = 0;
            return Err(io::Error::new(io::ErrorKind::Interrupted, "Retry"));
        }
        self.pos += n;
        self.tot += n;
        Ok(n)
    }

    pub fn clear(&mut self) {
        let (tx, rx) = mpsc::channel();
        self.tx = tx;
        self.rx = Mutex::new(rx);
        self.pos = 0;
        self.tot = 0;
        self.num = 0;
        self.file = None;
        self.mark_failed = false;
        self.completed = false;
    }
}

impl Default for Cat {
    fn default() -> Self {
        Self::new()
    }
}

impl io::Read for Cat {
    #[tracing::instrument(level = "trace", skip(buf))]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.completed {
            tracing::event!(
                tracing::Level::TRACE,
                action = "complete",
                total_bytes = self.tot,
                chunks = self.num
            );
            return Ok(0);
        }
        if let Some(mut file) = self.file.as_ref() {
            let n = file.read(buf)?;
            tracing::event!(
                tracing::Level::TRACE,
                action = "read",
                read_bytes = n,
                total_bytes = self.tot,
                chunks = self.num
            );
            return self.ok_or_retry(n);
        }
        let opt_path = {
            tracing::event!(
                tracing::Level::TRACE,
                action = "receive",
                total_bytes = self.tot,
                chunks = self.num
            );
            let rx = self.rx.lock().unwrap();
            rx.recv().map_err(channel_recv_error)?
        };
        if let Some(path) = opt_path {
            loop {
                tracing::event!(
                    tracing::Level::TRACE,
                    action = "open",
                    path = format!("{path:?}", path = path),
                    total_bytes = self.tot,
                    chunks = self.num
                );
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
                    tracing::event!(
                        tracing::Level::TRACE,
                        action = "read",
                        read_bytes = n,
                        total_bytes = self.tot,
                        chunks = self.num
                    );
                    self.file = Some(file);
                    self.ok_or_retry(n)
                });
            }
        } else {
            // self.file is None and received None from channel, just shutdown
            tracing::event!(
                tracing::Level::TRACE,
                action = "completed",
                total_bytes = self.tot,
                chunks = self.num
            );
            self.completed = true;
            Ok(0)
        }
    }
}
