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
use std::sync::mpsc::{RecvError, SendError};
use std::sync::Mutex;
use tempfile::TempDir;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};

use super::fragment::Fragment;
use super::notify::notify_error;

pub fn channel_send_error<T>(e: SendError<T>) -> io::Error {
    io::Error::other(format!("Channel send error: {e}"))
}

pub fn channel_recv_error(e: RecvError) -> io::Error {
    io::Error::other(format!("Channel recv error: {e}"))
}

pub fn tokio_send_error<T>(err: tokio::sync::mpsc::error::SendError<T>) -> io::Error {
    io::Error::other(format!("Tokio send error: {err}"))
}

pub fn tokio_recv_error(err: tokio::sync::mpsc::error::TryRecvError) -> io::Error {
    io::Error::other(format!("Tokio recv error: {err}"))
}

pub struct Watch {
    pub rx: Mutex<Receiver<notify::Result<Event>>>,
    pub watcher: RecommendedWatcher,
    pub shutdown: TempDir,
    handler: Option<Sender<Option<Fragment>>>,
}

impl Watch {
    pub fn new(handler: Option<Sender<Option<Fragment>>>) -> io::Result<Self> {
        // here we can shutdown the watch
        let shutdown = tempfile::tempdir()?;
        let (tx, rx) = mpsc::channel(10);
        let mut watcher = RecommendedWatcher::new(
            move |res| {
                futures::executor::block_on(async {
                    tx.send(res).await.unwrap();
                })
            },
            notify::Config::default(),
        )
        .map_err(notify_error)?;
        // let watcher monitor the shutdown path
        watcher
            .watch(shutdown.path(), RecursiveMode::NonRecursive)
            .map_err(notify_error)?;

        Ok(Self {
            rx: Mutex::new(rx),
            watcher,
            shutdown,
            handler,
        })
    }

    pub async fn notify(&self, fragment: Option<Fragment>) -> io::Result<()> {
        if let Some(h) = &self.handler {
            h.send(fragment).await.map_err(tokio_send_error)
        } else {
            Ok(())
        }
    }
}
