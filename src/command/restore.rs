// Copyright The Permafrust Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use crate::cli::{Cli, Restore};
use crate::core::cat::Cat;
use crate::core::channel::channel_send_error;
use crate::core::fragment::Fragment;
use crate::core::notify::notify_error;
use crate::core::path::{Queue, SpoolPathComponents};
use crate::crypto::openpgp::{
    build_decryptor, openpgp_error, read_password_fd, secret_key_store, SecretKeyStore,
};
use crossbeam::channel::{Receiver, Sender};
use notify::event::CreateKind;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use sequoia_openpgp::policy::StandardPolicy;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::os::unix::prelude::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::{fs, io, thread};

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

pub fn perform_restore(cli: &Cli, restore: &Restore) -> io::Result<()> {
    log::info!("RESTORE…");

    let output: Box<dyn io::Write> = build_writer(restore.output.as_ref())?;

    let spool_path_components = SpoolPathComponents::from_prefix(
        cli.spool.clone(),
        restore.vault,
        restore.prefix.clone().unwrap(),
    );
    let restore_dir = spool_path_components.to_queue_path(Queue::Restore)?;

    let (notify_sender, notify_receiver) = crossbeam::channel::bounded(10);

    let mut watcher =
        RecommendedWatcher::new(notify_sender, notify::Config::default()).map_err(notify_error)?;

    let policy = &StandardPolicy::new();
    let password = restore.pass_fd.and_then(read_password_fd);
    let secret_key_store = secret_key_store(policy, restore.keyring.iter().flatten(), password)?;

    //use_dir_atomic_create_maybe(&restore_dir, CreateDirectory::Recursive)?;

    log::trace!("Watching {restore_dir:?}");
    watcher
        .watch(&restore_dir, RecursiveMode::Recursive)
        .map_err(notify_error)?;

    let concat = Cat::new();
    let sender = concat.tx();

    let handle = thread::spawn(move || {
        notify_event_worker(&restore_dir, &mut watcher, &notify_receiver, &sender)
    });

    let copy_result = fragment_worker(concat, secret_key_store, policy, output)?;
    log::info!("Received total of {copy_result} bytes");

    handle.join().expect("could not join thread")
}

fn fragment_worker(
    concat: Cat,
    secret_key_store: SecretKeyStore,
    policy: &StandardPolicy,
    output: Box<dyn io::Write>,
) -> io::Result<u64> {
    log::trace!("Starting fragment_worker…");
    let mut buffered_writer = io::BufWriter::new(output);
    let reader = io::BufReader::new(concat);
    let mut decryptor = build_decryptor(secret_key_store, policy, reader).map_err(openpgp_error)?;
    let bytes_written = io::copy(&mut decryptor, &mut buffered_writer)?;
    log::trace!("Finishing fragment_worker…");
    Ok(bytes_written)
}

fn send_or_push_fragment(
    sender: &Sender<Option<PathBuf>>,
    heap: &mut BinaryHeap<Fragment>,
    fragment: Fragment,
    priority: Reverse<i32>,
) -> io::Result<Reverse<i32>> {
    if fragment.priority == priority {
        log::trace!("Sending fragment {fragment}");
        let Reverse(prio) = fragment.priority;
        sender
            .send(Some(fragment.path))
            .map_err(channel_send_error)?;
        Ok(Reverse(prio + 1))
    } else {
        log::debug!(
            "Ignoring fragment {fragment}, waiting for new fragment with priority {priority:?}"
        );
        heap.push(fragment);
        Ok(priority)
    }
}

fn notify_event_worker(
    backup_dir: &Path,
    watcher: &mut RecommendedWatcher,
    notify_receiver: &Receiver<Result<notify::Event, notify::Error>>,
    sender: &Sender<Option<PathBuf>>,
) -> io::Result<()> {
    log::trace!("Starting notify_event_worker…");

    let mut heap: BinaryHeap<Fragment> = BinaryHeap::new();
    let mut current_priority = Reverse(1);
    let mut zero_received: bool = false;

    for event in notify_receiver {
        match &event.map_err(notify_error)? {
            notify::Event { kind, paths, attrs }
                if kind == &EventKind::Create(CreateKind::Folder) =>
            {
                log::debug!("Received restore input path: {kind:?} {paths:?} {attrs:?}");
                for path in paths {
                    log::trace!("Watching path {path:?}, unwatching path {backup_dir:?}");
                    watcher
                        .watch(path, RecursiveMode::NonRecursive)
                        .map_err(notify_error)?;
                    watcher
                        .watch(backup_dir, RecursiveMode::NonRecursive)
                        .map_err(notify_error)?;
                }
            }
            notify::Event {
                kind, paths, attrs, ..
            } if kind == &EventKind::Create(CreateKind::File) => {
                for path in paths {
                    if let Ok(metadata) = fs::metadata(path) {
                        let nlink = metadata.nlink();
                        if nlink > 1 {
                            log::trace!(
                                "Found hard-linked file ({nlink:?}): {kind:?} {paths:?} {attrs:?}"
                            );
                        }
                    }
                    let Some(current_fragment) = Fragment::new(path.as_path()) else {
                        continue;
                    };
                    if current_fragment.priority == Reverse(0) {
                        log::trace!("Received zero fragment: {current_fragment:?}");
                        zero_received = true;
                        continue;
                    }
                    current_priority = send_or_push_fragment(
                        sender,
                        &mut heap,
                        current_fragment,
                        current_priority,
                    )?;
                }
            }
            notify::Event {
                kind, paths, attrs, ..
            } => {
                log::trace!("Ignoring event {kind:?} {paths:?} {attrs:?}");
            }
        }
        loop {
            let Some(min_fragment) = heap.pop() else {
                break; // empty heap
            };
            let next_priority =
                send_or_push_fragment(sender, &mut heap, min_fragment, current_priority)?;
            if next_priority == current_priority {
                break; // we need to wait for the next fragment with current_priority
            } else {
                current_priority = next_priority; // we found a fragment, let's search for next_priority
            }
        }
        if zero_received {
            // we found the zero file, signal shutdown
            sender.send(None).map_err(channel_send_error)?;
            break;
        }
    }
    log::trace!("Finishing notify_event_worker…");
    Ok(())
}
