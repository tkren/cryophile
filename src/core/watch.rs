// Copyright The Cryophile Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::io;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, RecvError, SendError};
use std::sync::{mpsc, Mutex};
use tempfile::TempDir;
use tokio::sync::mpsc::Sender;

use super::notify::notify_error;

pub fn channel_send_error<T>(e: SendError<T>) -> io::Error {
    io::Error::other(format!("Channel send error: {e}"))
}

pub fn channel_recv_error(e: RecvError) -> io::Error {
    io::Error::other(format!("Channel recv error: {e}"))
}

pub struct Watch {
    pub rx: Mutex<Receiver<notify::Result<Event>>>,
    pub watcher: RecommendedWatcher,
    pub shutdown: TempDir,
    _handler: Option<Sender<Option<PathBuf>>>,
}

impl Watch {
    pub fn new(handler: Option<Sender<Option<PathBuf>>>) -> io::Result<Self> {
        // here we can shutdown the watch
        let shutdown = tempfile::tempdir()?;
        let (tx, rx) = mpsc::channel();
        let mut watcher =
            RecommendedWatcher::new(tx, notify::Config::default()).map_err(notify_error)?;
        // let watcher monitor the shutdown path
        watcher
            .watch(shutdown.path(), RecursiveMode::NonRecursive)
            .map_err(notify_error)?;

        Ok(Self {
            rx: Mutex::new(rx),
            watcher,
            shutdown,
            _handler: handler,
        })
    }
}
