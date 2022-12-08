use crate::cli::{Cli, Restore};
use crate::core::fragment::Fragment;
use crate::core::notify::notify_error;
use crate::core::path::BackupPathComponents;
use crossbeam::channel::{Receiver, Sender};
use notify::event::{AccessKind, AccessMode, CreateKind};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::path::{Path, PathBuf};
use std::{fs, io, thread};

pub fn perform_restore(cli: &Cli, restore: &Restore) -> io::Result<()> {
    log::info!("RESTORE…");

    let _output: Box<dyn io::Write> = match &restore.output {
        Some(p) if p.as_path() == Path::new("-") => {
            log::info!("Writing to stdout…");
            Box::new(io::stdout())
        }
        None => {
            log::info!("Writing to stdout…");
            Box::new(io::stdout())
        }
        Some(output) => {
            log::info!("Opening {output:?}…");
            Box::new(fs::File::open(output)?)
        }
    };

    let backup_path_components: BackupPathComponents =
        (cli.spool.clone(), restore.vault, restore.output.clone()).into();

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

    thread::spawn(move || fragment_worker(fragment_receiver));

    notify_event_worker(&backup_dir, &mut watcher, &notify_receiver, &event_sender)?;

    Ok(())
}

fn channel_send_error<T>(e: crossbeam::channel::SendError<T>) -> io::Error {
    io::Error::new(io::ErrorKind::Other, format!("Channel send error: {e}"))
}

fn channel_recv_error(e: crossbeam::channel::RecvError) -> io::Error {
    io::Error::new(io::ErrorKind::Other, format!("Channel recv error: {e}"))
}

fn fragment_worker(receiver: crossbeam::channel::Receiver<Option<Fragment>>) -> io::Result<()> {
    log::trace!("Starting fragment_worker…");
    loop {
        match receiver.recv().map_err(channel_recv_error)? {
            None => break,
            Some(item) => {
                log::trace!("Received fragment {item}");
            }
        }
    }
    log::trace!("Finishing fragment_worker…");
    Ok(())
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
    log::trace!("Starting notify_event_worker");

    let mut heap: BinaryHeap<Fragment> = BinaryHeap::new();
    let mut current_priority = Reverse(1);

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
                    current_priority = send_or_push_fragment(
                        sender,
                        &mut heap,
                        &current_fragment,
                        current_priority,
                    )?;
                }
            }
            notify::Event { kind, paths, attrs } => {
                log::trace!("Ignoring event {kind:?} {paths:?} {attrs:?}");
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
    }
    log::trace!("Finishing notify_event_worker…");
    Ok(())
}
