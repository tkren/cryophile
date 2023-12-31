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
use crate::core::notify::notify_error;
use crate::core::path::{Queue, SpoolPathComponents};
use crate::Config;
use notify::event::{AccessKind, AccessMode, CreateKind, RemoveKind};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc;
use std::{fs, io};
use walkdir::WalkDir;

pub fn perform_freeze(config: &Config, _freeze: &Freeze) -> io::Result<()> {
    log::info!("FREEZE…");

    let aws_config_future = aws::aws_config(None);
    let aws_config = futures::executor::block_on(aws_config_future);
    log::trace!(
        "Using AWS config region {region:?}",
        region = aws_config.region()
    );

    let aws_client_future = aws::aws_client(&aws_config);
    let aws_client = futures::executor::block_on(aws_client_future);
    log::trace!("Using AWS client {aws_client:?}");

    let (tx, rx) = mpsc::channel();

    let mut watcher =
        RecommendedWatcher::new(tx, notify::Config::default()).map_err(notify_error)?;

    let spool_path_components = SpoolPathComponents::from_spool(config.cli.spool.clone());
    let freeze_dir = spool_path_components.to_queue_path(Queue::Freeze)?;

    watch_read_dir(&mut watcher, &freeze_dir, RecursiveMode::Recursive)?;
    log::debug!("Watching spool {freeze_dir:?}");

    for res in rx {
        event_handler(res, &freeze_dir, &mut watcher).map_err(notify_error)?;
    }

    Ok(())
}

fn watch_read_dir(
    watcher: &mut notify::RecommendedWatcher,
    path: &Path,
    mode: RecursiveMode,
) -> io::Result<()> {
    if !path.is_dir() {
        log::warn!("Ignoring non-directory: {path:?}");
        return Ok(());
    }

    watcher.watch(path, mode).map_err(notify_error)?;
    log::debug!("Watching path ({mode:?}): {path:?}");

    for entry in WalkDir::new(path) {
        if let Err(e) = &entry {
            log::warn!("Cannot walk {entry:?}, ignoring: {e}");
            continue;
        } else if let Ok(dir_entry) = &entry {
            if !dir_entry.path_is_symlink() {
                let dir_entry_path = dir_entry.path();
                if dir_entry_path.is_file() {
                    log::debug!("Found {dir_entry_path:?}");
                    // TODO found file may or may not be open for writing
                    continue;
                }
            }
            log::debug!("Ignoring {dir_entry:?}");
        }
    }

    for entry in fs::read_dir(path)? {
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

    Ok(())
}

fn event_handler(
    result: Result<notify::Event, notify::Error>,
    spool: &Path,
    _watcher: &mut notify::RecommendedWatcher,
) -> Result<(), notify::Error> {
    // TODO check which level in spool causes the event
    // TODO inside vault: new backup dirs arrive, add them if they are not yet uploaded, if uploaded unwatch backup_dir
    // TODO if chunk.0 file arrives, backup is done, do another read_dir for files that are not in "w" mode
    match result {
        Ok(notify::Event { kind, paths, attrs }) if kind == EventKind::Create(CreateKind::File) => {
            log::info!("Create file event: {kind:?} {paths:?} {attrs:?}");
            for path in &paths {
                if let Some(parent) = path.parent() {
                    if parent == spool {
                        //watch_read_dir(watcher, &path, RecursiveMode::Recursive)?;
                        log::debug!(
                            "Ignoring spool create file event: {kind:?} {paths:?} {attrs:?}"
                        );
                    } else {
                        log::debug!("Ignoring watched file event: {kind:?} {paths:?} {attrs:?}");
                    }
                } else {
                    log::debug!("Ignoring create root file event: {kind:?} {paths:?} {attrs:?}");
                }
            }
        }
        Ok(notify::Event { kind, paths, attrs })
            if kind == EventKind::Access(AccessKind::Close(AccessMode::Write)) =>
        {
            log::info!("Close file event: {kind:?} {paths:?} {attrs:?}");
            for path in &paths {
                if let Some(parent) = path.parent() {
                    if parent == spool {
                        //watch_read_dir(watcher, &path, RecursiveMode::Recursive)?;
                        log::debug!(
                            "Ignoring spool close file event: {kind:?} {paths:?} {attrs:?}"
                        );
                    } else {
                        log::debug!("Ignoring watched close event: {kind:?} {paths:?} {attrs:?}");
                    }
                } else {
                    log::debug!("Ignoring create root close event: {kind:?} {paths:?} {attrs:?}");
                }
            }
        }
        Ok(notify::Event { kind, paths, attrs })
            if kind == EventKind::Create(CreateKind::Folder) =>
        {
            log::info!("Create folder event: {kind:?} {paths:?} {attrs:?}");
            for path in &paths {
                if let Some(parent) = path.parent() {
                    if parent == spool {
                        //watch_read_dir(watcher, &path, RecursiveMode::Recursive)?;
                        log::debug!(
                            "Ignoring spool create folder event: {kind:?} {paths:?} {attrs:?}"
                        );
                    } else {
                        log::debug!("Ignoring watched folder event: {kind:?} {paths:?} {attrs:?}");
                    }
                } else {
                    log::debug!("Ignoring create root folder event: {kind:?} {paths:?} {attrs:?}");
                }
            }
        }
        Ok(notify::Event { kind, paths, attrs })
            if kind == EventKind::Remove(RemoveKind::Folder) =>
        {
            log::info!("Remove folder event: {kind:?} {paths:?} {attrs:?}");
            for path in &paths {
                if let Some(parent) = path.parent() {
                    if parent == spool {
                        //watch_read_dir(watcher, &path, RecursiveMode::Recursive)?;
                        log::debug!(
                            "Ignoring spool remove folder event: {kind:?} {paths:?} {attrs:?}"
                        );
                    } else {
                        log::debug!(
                            "Ignoring watched remove folder event: {kind:?} {paths:?} {attrs:?}"
                        );
                    }
                } else {
                    log::debug!("Ignoring remove root folder event: {kind:?} {paths:?} {attrs:?}");
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
    Ok(())
}
