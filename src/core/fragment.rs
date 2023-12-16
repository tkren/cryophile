// Copyright The Permafrust Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use std::{
    cmp::{Ordering, Reverse},
    collections::BinaryHeap,
    fmt, io,
    path::PathBuf,
};

use crossbeam::channel::Sender;

use super::channel::channel_send_error;

#[derive(Clone, Debug, Eq)]
pub struct Fragment {
    pub priority: Reverse<i32>,
    pub path: PathBuf,
}

impl fmt::Display for Fragment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{priority:?} {path:?}",
            priority = self.priority,
            path = self.path
        )
    }
}

impl Ord for Fragment {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority.cmp(&other.priority)
    }
}

impl PartialOrd for Fragment {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Fragment {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}

impl Fragment {
    pub fn new(path: PathBuf) -> Option<Self> {
        let Some(extension) = path.extension() else {
            return None;
        };
        let Some(priority) = extension.to_str()?.parse::<i32>().ok() else {
            return None;
        };
        Some(Self {
            priority: Reverse(priority),
            path,
        })
    }

    pub fn is_zero(&self) -> bool {
        self.priority.0 == 0
    }
}

#[derive(Debug)]
pub struct FragmentQueue {
    sender: Sender<Option<PathBuf>>,
    heap: BinaryHeap<Fragment>,
    current: Reverse<i32>,
    zero: bool,
}

impl FragmentQueue {
    pub fn new(sender: Sender<Option<PathBuf>>) -> Self {
        Self {
            sender,
            heap: BinaryHeap::new(),
            current: Reverse(1),
            zero: false,
        }
    }

    pub fn send(&mut self, fragment: Fragment) -> io::Result<bool> {
        if fragment.is_zero() {
            log::trace!("Received zero fragment: {fragment:?}");
            self.zero = true;
            return Ok(false);
        }
        if fragment.priority == self.current {
            log::trace!("Sending fragment {fragment}");
            self.sender
                .send(Some(fragment.path))
                .map_err(channel_send_error)?;
            self.current = Reverse(fragment.priority.0 + 1);
            Ok(true)
        } else {
            log::debug!(
                "Ignoring fragment {fragment}, waiting for new fragment with priority {priority:?}",
                priority = self.current
            );
            self.heap.push(fragment);
            Ok(false)
        }
    }

    pub fn send_backlog(&mut self) -> io::Result<()> {
        // empty heap
        while let Some(min_fragment) = self.heap.pop() {
            if !self.send(min_fragment)? {
                break; // we need to wait for the next fragment with current_priority
            };
        }
        Ok(())
    }

    pub fn send_zero_maybe(&mut self) -> io::Result<bool> {
        if !self.zero {
            return Ok(false);
        }
        if self.heap.is_empty() {
            // we found the zero file, signal shutdown
            self.sender.send(None).map_err(channel_send_error)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
