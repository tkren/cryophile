// Copyright The Cryophile Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use crate::cli::Restore;
use crate::compression::decompressor::Decompressor;
use crate::compression::CompressionType;
use crate::core::cat::Cat;
use crate::core::fragment::FragmentQueue;
use crate::core::notify::notify_error;
use crate::core::path::{CreateDirectory, Queue, SpoolPathComponents};
use crate::core::watch::Watch;
use crate::crypto::openpgp::{
    build_decryptor, openpgp_error, read_password_fd, secret_key_store, SecretKeyStore,
};
use crate::Config;
use notify::event::CreateKind;
use notify::{EventKind, RecursiveMode, Watcher};
use sequoia_openpgp::policy::StandardPolicy;
use std::os::unix::prelude::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;
use std::{fs, io, thread};
use walkdir::WalkDir;

pub fn perform_restore(config: &Config, restore: &Restore) -> io::Result<()> {
    log::info!("RESTORE…");

    let output: Box<dyn io::Write> = build_writer(restore.output.as_ref())?;

    let spool_path_components = SpoolPathComponents::new(
        config.cli.spool.clone(),
        restore.vault,
        restore.prefix.clone(),
        restore.ulid,
    );

    let concat = Cat::new();
    let fragment_queue = FragmentQueue::new(concat.tx());

    let watch = Box::new(Watch::new()?);

    // Create and watch restore directory, or use restore directory from a previous run.
    // No need to watch once we could fully walked the downloaded restore directory (e.g., if restore was interrupted).
    let handle = walk_or_watch_restore_dir(&spool_path_components, watch, fragment_queue)?;

    let policy = &StandardPolicy::new();
    // TODO use optional CRYOPHILE_ASKPASS instead of terminal prompt
    // TODO batch mode should not try to prompt for password at all
    let password = restore.pass_fd.and_then(read_password_fd);
    let secret_key_store = secret_key_store(policy, restore.keyring.iter().flatten(), password)?;

    let copy_result = fragment_worker(
        concat,
        secret_key_store,
        policy,
        restore.compression,
        output,
    )?;
    log::info!("Received total of {copy_result} bytes");

    handle
        .map(|h| h.join().expect("could not join thread"))
        .unwrap_or(Ok(()))
}

fn build_writer(path: Option<&PathBuf>) -> io::Result<Box<dyn io::Write>> {
    let writer: Box<dyn io::Write> = match path {
        Some(p) if p.as_path() == Path::new("-") => {
            log::info!("Writing to stdout…");
            Box::new(io::stdout())
        }
        None => {
            log::info!("Writing to stdout…");
            Box::new(io::stdout())
        }
        Some(output) => {
            log::info!("Creating restore output {output:?}");
            Box::new(
                fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(output)?,
            )
        }
    };
    Ok(writer)
}

fn walk_or_watch_restore_dir(
    spool_path_components: &SpoolPathComponents,
    watch: Box<Watch>,
    mut queue: FragmentQueue,
) -> io::Result<Option<JoinHandle<io::Result<()>>>> {
    let path =
        match spool_path_components.with_queue_path(Queue::Restore, CreateDirectory::Recursive) {
            Ok(path) => {
                // we could create path, now watch for incoming files
                let handle = watch_restore_dir(path.as_path(), watch, queue)?;
                return Ok(Some(handle));
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                let path = spool_path_components.to_queue_path(Queue::Restore)?;
                if !path.is_dir() {
                    return Err(err); // a non-directory is in the way, just bail out
                }
                path // reuse directory and walk
            }
            Err(err) => {
                return Err(err);
            }
        };

    // enter path, only retrieving direct children
    let walk = WalkDir::new(path.as_path())
        .follow_root_links(false)
        .min_depth(1)
        .max_depth(1);

    for entry in walk {
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
                    queue.send_path(dir_entry_path.to_path_buf())?;
                    continue;
                }
                log::debug!("Ignoring path {dir_entry:?}");
            }
        }
    }
    queue.send_backlog()?;
    if queue.send_zero_maybe()? {
        Ok(None)
    } else {
        let handle = watch_restore_dir(path.as_path(), watch, queue)?;
        Ok(Some(handle))
    }
}

fn watch_restore_dir(
    path: &Path,
    mut watch: Box<Watch>,
    queue: FragmentQueue,
) -> io::Result<JoinHandle<io::Result<()>>> {
    log::trace!("Watching {path:?}");
    watch
        .watcher
        .watch(path, RecursiveMode::NonRecursive)
        .map_err(notify_error)?;

    let handle = thread::spawn(move || notify_event_worker(&watch, queue));
    Ok(handle)
}

fn notify_event_worker(watch: &Watch, mut queue: FragmentQueue) -> io::Result<()> {
    log::trace!("Starting notify_event_worker…");
    let notify_receiver = watch.rx.lock().expect("Cannot lock watch receiver");
    for event in notify_receiver.iter() {
        match event.map_err(notify_error)? {
            notify::Event {
                kind: EventKind::Create(CreateKind::File),
                paths,
                ..
            } => {
                for path in paths {
                    if path.is_symlink() {
                        log::warn!("Ignoring symlink {path:?}");
                        continue;
                    }
                    queue.send_path(path)?;
                }
            }
            notify::Event {
                kind, paths, attrs, ..
            } => {
                log::trace!("Ignoring event {kind:?} {paths:?} {attrs:?}");
            }
        }
        queue.send_backlog()?;
        if queue.send_zero_maybe()? {
            break;
        };
    }
    log::trace!("Finishing notify_event_worker…");
    Ok(())
}

fn fragment_worker(
    concat: Cat,
    secret_key_store: SecretKeyStore,
    policy: &StandardPolicy,
    compression: Option<CompressionType>,
    mut output: Box<dyn io::Write>,
) -> io::Result<u64> {
    log::trace!("Starting fragment_worker…");
    let reader = io::BufReader::new(concat);
    let decryptor = build_decryptor(secret_key_store, policy, reader).map_err(openpgp_error)?;
    // guess compression algorithm by default
    let mut decompressor = Decompressor::new(decryptor);
    if let Some(compression_type) = compression {
        // force decompression with compression_type
        log::info!("Decompressing restore stream with {compression_type:?}…");
        decompressor = decompressor.with_compression(compression_type);
    } else {
        log::info!("Guessing decompression algorithm from restore stream…");
    }
    let bytes_written = decompressor.copy_to(&mut output)?;
    log::trace!("Finishing fragment_worker…");
    Ok(bytes_written)
}
