use crate::cli::{Cli, Restore};
use crate::core::fragment::Fragment;
use crate::core::notify::notify_error;
use crate::core::path::BackupPathComponents;
use crossbeam::channel::{Receiver, Sender};
use notify::event::{AccessKind, AccessMode, CreateKind};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::os::unix::prelude::OpenOptionsExt;
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

    let backup_path_components: BackupPathComponents =
        (cli.spool.clone(), restore.vault, restore.prefix.clone()).into();

    let backup_dir: Option<PathBuf> = (&backup_path_components).into();
    let Some(backup_dir) = backup_dir else {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("Invalid path {backup_path_components:?} given")));
    };

    let (notify_sender, notify_receiver) = crossbeam::channel::bounded(10);

    let mut watcher =
        RecommendedWatcher::new(notify_sender, notify::Config::default()).map_err(notify_error)?;

    log::trace!("Watching {backup_dir:?}");
    watcher
        .watch(&backup_dir, RecursiveMode::Recursive)
        .map_err(notify_error)?;

    let (event_sender, fragment_receiver) = crossbeam::channel::unbounded::<Option<Fragment>>();

    thread::spawn(move || {
        notify_event_worker(&backup_dir, &mut watcher, &notify_receiver, &event_sender)
    });

    let copy_result = fragment_worker(fragment_receiver, output)?;
    log::info!("Received total of {copy_result} bytes");

    Ok(())
}

fn channel_send_error<T>(e: crossbeam::channel::SendError<T>) -> io::Error {
    io::Error::new(io::ErrorKind::Other, format!("Channel send error: {e}"))
}

fn channel_recv_error(e: crossbeam::channel::RecvError) -> io::Error {
    io::Error::new(io::ErrorKind::Other, format!("Channel recv error: {e}"))
}

fn fragment_worker(
    receiver: crossbeam::channel::Receiver<Option<Fragment>>,
    output: Box<dyn io::Write>,
) -> io::Result<u64> {
    log::trace!("Starting fragment_worker…");
    let mut bytes_written: u64 = 0;
    let mut buffered_writer = io::BufWriter::new(output);
    loop {
        match receiver.recv().map_err(channel_recv_error)? {
            None => break,
            Some(item) => {
                log::trace!("Received fragment {item}");
                let fragment = fs::OpenOptions::new().read(true).open(item.path)?;
                let mut reader = io::BufReader::new(fragment);
                bytes_written += io::copy(&mut reader, &mut buffered_writer)?
            }
        }
    }
    log::trace!("Finishing fragment_worker…");
    Ok(bytes_written)
}

fn send_or_push_fragment(
    sender: &Sender<Option<Fragment>>,
    heap: &mut BinaryHeap<Fragment>,
    fragment: &Fragment,
    priority: Reverse<i32>,
) -> io::Result<Reverse<i32>> {
    if fragment.priority == priority {
        log::trace!("Sending fragment {fragment}");
        let Reverse(prio) = fragment.priority;
        sender
            .send(Some(fragment.to_owned()))
            .map_err(channel_send_error)?;
        Ok(Reverse(prio + 1))
    } else {
        log::debug!(
            "Ignoring fragment {fragment}, waiting for new fragment with priority {priority:?}"
        );
        heap.push(fragment.to_owned());
        Ok(priority)
    }
}

fn notify_event_worker(
    backup_dir: &Path,
    watcher: &mut RecommendedWatcher,
    notify_receiver: &Receiver<Result<notify::Event, notify::Error>>,
    sender: &Sender<Option<Fragment>>,
) -> io::Result<()> {
    log::trace!("Starting notify_event_worker…");

    let mut heap: BinaryHeap<Fragment> = BinaryHeap::new();
    let mut current_priority = Reverse(1);
    let mut zero_received: bool = false;

    for result in notify_receiver {
        match &result.map_err(notify_error)? {
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
            notify::Event { kind, paths, attrs }
                if kind == &EventKind::Access(AccessKind::Close(AccessMode::Write)) =>
            {
                log::debug!("Received restore fragment: {kind:?} {paths:?} {attrs:?}");
                for path in paths {
                    let Some(current_fragment) = Fragment::new(path.as_path()) else {continue;};
                    if current_fragment.priority == Reverse(0) {
                        log::trace!("Received zero fragment: {current_fragment:?}");
                        zero_received = true;
                        continue;
                    }
                    current_priority = send_or_push_fragment(
                        sender,
                        &mut heap,
                        &current_fragment,
                        current_priority,
                    )?;
                }
            }
            notify::Event { .. } => {
                //log::trace!("Ignoring event {kind:?} {paths:?} {attrs:?}");
                continue;
            }
        }
        loop {
            let Some(min_fragment) = heap.pop() else {
                break; // empty heap
            };
            let next_priority =
                send_or_push_fragment(sender, &mut heap, &min_fragment, current_priority)?;
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
