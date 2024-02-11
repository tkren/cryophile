// Copyright The Cryophile Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use crate::cli::Freeze;
use crate::core::aws;
use crate::core::backup_id::BackupId;
use crate::core::fragment::{Fragment, Interval, IntervalSet};
use crate::core::notify::notify_error;
use crate::core::path::{CreateDirectory, Queue, SpoolPathComponents};
use crate::core::watch::Watch;
use crate::Config;
use futures::FutureExt;
use notify::{event::CreateKind, EventKind, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::thread::{self, JoinHandle};
use std::{fs, io};
use tokio::fs::OpenOptions;
use tokio::runtime::Builder;
use tokio::signal;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::task::JoinSet;
use walkdir::WalkDir;

pub fn perform_freeze(config: &Config, freeze: &Freeze) -> io::Result<()> {
    log::info!("FREEZE…");

    let runtime = Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()?;

    let js = JoinSet::new();

    let (upload_tx, upload_rx) = mpsc::channel::<Option<Fragment>>(32);

    let watch = Box::new(Watch::new(Some(upload_tx.clone()))?);
    let shutdown_path = PathBuf::from(watch.shutdown.path());

    let spool = config.cli.spool.clone();
    let prefix_str_maybe = freeze.prefix.as_ref().and_then(|path| path.to_str());

    let _watch_handle = runtime.spawn(match (freeze.vault, freeze.ulid) {
        (Some(vault), Some(ulid)) => {
            let spool_path_components =
                SpoolPathComponents::new(spool, BackupId::new(vault, prefix_str_maybe, ulid));
            walk_or_watch_freeze_dir(&spool_path_components, watch, RecursiveMode::NonRecursive)
        }
        (_, _) => {
            let spool_path_components = SpoolPathComponents::from_spool(spool);
            walk_or_watch_freeze_dir(&spool_path_components, watch, RecursiveMode::Recursive)
        }
    });

    // TODO upload incoming files to S3
    // https://docs.aws.amazon.com/AmazonS3/latest/userguide/mpuoverview.html
    let freezer_handle = runtime.spawn(freezer(upload_rx));

    let _sigint_handle = runtime.spawn(sigint_handler(upload_tx.clone(), shutdown_path));

    let freezer_result = runtime.block_on(freezer_handle).map_err(|err| {
        log::error!("Cannot join aws handle: {err}");
        io::Error::other(format!("join error: {err}"))
    })?;

    _watch_handle
        .map(|h| h.join().expect("could not join thread"))
        .unwrap_or(freezer_result)
}

async fn sigint_handler(
    upload_tx: Sender<Option<Fragment>>,
    mut shutdown: PathBuf,
) -> io::Result<()> {
    match signal::ctrl_c().await {
        Ok(()) => {
            log::info!("Received SIGINT, shutting down…");
            upload_tx.send(None).await.map_err(|err| {
                log::error!("Cannot send to freezer: {err}");
                io::Error::other(format!("Freezer send error: {err}"))
            })?;
            shutdown.push("shutdown");
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(shutdown)
                .await?;
            Ok(())
        }
        Err(err) => {
            log::error!("Unable to listen for shutdown signal: {err}");
            Err(err)
        }
    }
}

async fn freezer(mut rx: Receiver<Option<Fragment>>) -> io::Result<()> {
    log::trace!("Starting freezer…");
    let aws_config = aws::aws_config(None).await;
    let _client = aws::aws_client(&aws_config).await;
    while let Some(path_maybe) = rx.recv().await {
        match path_maybe {
            Some(path) => {
                log::info!("Freezing {path:?}");
            }
            None => {
                log::trace!("Received shutdown request");
                break;
            }
        }
    }
    log::trace!("Shutdown freezer…");
    Ok(())
}

async fn walk_or_watch_freeze_dir(
    spool_path_components: &SpoolPathComponents<'_>,
    watch: Box<Watch>,
    mode: RecursiveMode,
) -> io::Result<Option<JoinHandle<io::Result<()>>>> {
    let path =
        match spool_path_components.with_queue_path(Queue::Freeze, CreateDirectory::Recursive) {
            Ok(path) => {
                // we could create path, now watch for incoming files
                // TODO makes sense??
                let handle = watch_freeze_dir(path, watch)?;
                return Ok(Some(handle));
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                let path = spool_path_components.to_queue_path(Queue::Freeze)?;
                if !path.is_dir() {
                    return Err(err); // a non-directory is in the way, just bail out
                }
                path // reuse directory and walk
            }
            Err(err) => {
                return Err(err);
            }
        };

    // recursively walk directory
    let mut walker = WalkDir::new(path.clone())
        .follow_root_links(false)
        .min_depth(1);

    if mode == RecursiveMode::NonRecursive {
        walker = walker.max_depth(1);
    }

    let mut intervals = IntervalSet::new();

    for entry in walker {
        match &entry {
            Err(e) => {
                log::warn!("Cannot walk {entry:?}, ignoring: {e}");
                continue;
            }
            Ok(dir_entry) => {
                if dir_entry.path_is_symlink() {
                    log::debug!("Ignoring symlink {dir_entry:?}");
                    continue;
                }
                let dir_entry_path = dir_entry.path();
                if dir_entry_path.is_file() {
                    log::trace!("Found file {dir_entry_path:?}");

                    let frag = Fragment::new(dir_entry_path.to_path_buf());
                    if let Some(ref fragment) = frag {
                        let index = fragment.index();
                        intervals.insert(Interval::point(index));
                        watch.notify(frag).await?
                    }

                    log::debug!(
                        "Intervals ({len}): {last:?}",
                        len = intervals.len(),
                        last = intervals.last()
                    );
                    continue;
                } else if dir_entry_path.is_dir() {
                    log::trace!("Found directory {dir_entry_path:?}");
                    // watch?
                    continue;
                }
                log::debug!("Ignoring path {dir_entry:?}");
            }
        }
    }

    if mode == RecursiveMode::Recursive {
        for entry in fs::read_dir(path.clone())? {
            let entry = entry?;
            let sub_path = entry.path();
            if sub_path.is_dir() {
                // TODO only watch configured vaults
                //watcher
                //    .watch(sub_path.as_path(), mode)
                //    .map_err(notify_error)?;
                log::trace!("Not watching subdirectory ({mode:?}): {sub_path:?}")
                // TODO read_dir vault
            } else {
                log::warn!("Ignoring non-directory: {sub_path:?}")
            }
        }
    }

    if intervals.len() == 1 && intervals.last().is_some_and(|x| x.start == 0) {
        log::trace!("Found all freeze fragments of {path:?}, sending shutdown request");
        return Ok(None);
    }
    log::trace!("Resuming unfinished freeze of {path:?}");
    let handle = watch_freeze_dir(path, watch)?;
    Ok(Some(handle))
}

fn watch_freeze_dir(
    path: PathBuf,
    mut watch: Box<Watch>,
) -> io::Result<JoinHandle<io::Result<()>>> {
    log::trace!("Watching {path:?}");
    watch
        .watcher
        .watch(&path, RecursiveMode::NonRecursive)
        .map_err(notify_error)?;

    let handle = thread::spawn(move || notify_event_worker(&path, &watch));
    Ok(handle)
}

fn notify_event_worker(root: &Path, watch: &Watch) -> io::Result<()> {
    // TODO check which level in spool causes the event
    // TODO inside vault: new backup dirs arrive, add them if they are not yet uploaded, if uploaded unwatch backup_dir
    // TODO if chunk.0 file arrives, backup is done, do another read_dir for files that are not in "w" mode
    let shutdown_path = watch.shutdown.path();
    log::trace!("Starting notify_event_worker ({shutdown_path:?})…");
    let notify_receiver = watch.rx.lock().expect("Cannot lock watch receiver");
    'receiver_loop: for result in notify_receiver.iter() {
        match result.map_err(notify_error) {
            Ok(notify::Event { kind, paths, attrs })
                if kind == EventKind::Create(CreateKind::File) =>
            {
                log::debug!("Create file event: {kind:?} {paths:?} {attrs:?}");
                for path in &paths {
                    if let Some(parent) = path.parent() {
                        if parent == root {
                            //watch_read_dir(watcher, &path, RecursiveMode::Recursive)?;
                            log::debug!(
                                "Ignoring spool create file event: {kind:?} {paths:?} {attrs:?}"
                            );
                        } else if parent == shutdown_path {
                            log::debug!(
                                "Received shutdown file event: {kind:?} {path:?} {attrs:?}"
                            );
                            break 'receiver_loop;
                        } else {
                            log::debug!(
                                "Ignoring watched file event: {kind:?} {paths:?} {attrs:?}"
                            );
                        }
                    } else {
                        log::debug!(
                            "Ignoring create root file event: {kind:?} {paths:?} {attrs:?}"
                        );
                    }
                }
            }
            Ok(notify::Event { kind, paths, attrs })
                if kind == EventKind::Create(CreateKind::Folder) =>
            {
                log::debug!("Create folder event: {kind:?} {paths:?} {attrs:?}");
                for path in &paths {
                    if let Some(parent) = path.parent() {
                        if parent == root {
                            log::debug!(
                                "Ignoring spool create folder event: {kind:?} {paths:?} {attrs:?}"
                            );
                        } else {
                            log::debug!(
                                "Ignoring watched folder event: {kind:?} {paths:?} {attrs:?}"
                            );
                        }
                    } else {
                        log::debug!(
                            "Ignoring create root folder event: {kind:?} {paths:?} {attrs:?}"
                        );
                    }
                }
            }
            Ok(notify::Event { kind, paths, attrs }) => {
                log::debug!("Event: {kind:?} {paths:?} {attrs:?}");
            }
            Err(err) => {
                log::error!("watch error: {err:?}");
                return Err(err);
            }
        };
    }
    log::trace!("Shutdown notify_event_worker…");
    Ok(())
}
