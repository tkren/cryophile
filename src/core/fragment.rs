// Copyright The Cryophile Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use std::{
    cmp::{Ordering, Reverse},
    collections::{BTreeSet, BinaryHeap},
    fmt, io,
    ops::{Range, RangeBounds},
    path::PathBuf,
};

use std::sync::mpsc::Sender;

use super::watch::channel_send_error;

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
        let priority = path.extension()?.to_str()?.parse::<i32>().ok()?;
        Some(Self {
            priority: Reverse(priority),
            path,
        })
    }

    pub fn is_zero(&self) -> bool {
        self.priority.0 == 0
    }

    pub fn index(&self) -> i32 {
        self.priority.0
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

    pub fn send_path(&mut self, path: PathBuf) -> io::Result<bool> {
        Fragment::new(path)
            .map(|frag| self.send(frag))
            .unwrap_or(Ok(false))
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

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Interval {
    pub start: i32,
    pub end: i32,
}

impl Interval {
    pub fn new(start: i32, end: i32) -> Self {
        if end < start {
            Self {
                start: end,
                end: start,
            }
        } else {
            Self { start, end }
        }
    }

    pub fn point(p: i32) -> Self {
        Self { start: p, end: p }
    }

    pub fn from_range(r: Range<i32>) -> Self {
        Interval::new(r.start, r.end - 1)
    }

    pub fn envelope(&self, left: &Interval, right: &Interval) -> Self {
        Interval::new(self.start.min(left.start), self.end.max(right.end))
    }
}

impl fmt::Debug for Interval {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "[")?;
        self.start.fmt(fmt)?;
        write!(fmt, "..")?;
        self.end.fmt(fmt)?;
        write!(fmt, "]")?;
        Ok(())
    }
}

impl RangeBounds<i32> for Interval {
    fn start_bound(&self) -> std::ops::Bound<&i32> {
        std::ops::Bound::Included(&self.start)
    }

    fn end_bound(&self) -> std::ops::Bound<&i32> {
        std::ops::Bound::Included(&self.end)
    }
}

impl Ord for Interval {
    fn cmp(&self, other: &Self) -> Ordering {
        // self.start <= self.end && other.start <= other.end
        if self.contains(&other.start) && self.contains(&other.end) {
            // self.start <= other.start && other.end <= self.end
            Ordering::Equal
        } else if other.end < self.start {
            Ordering::Less
        } else if self.end < other.start {
            Ordering::Greater
        } else {
            // other.start < self.start && self.end < other.end
            Ordering::Equal
        }
    }
}

impl PartialOrd for Interval {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Default)]
pub struct IntervalSet {
    intervals: BTreeSet<Interval>,
}

impl fmt::Debug for IntervalSet {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "{{")?;
        self.intervals.fmt(fmt)?;
        write!(fmt, "}}")?;
        Ok(())
    }
}

impl IntervalSet {
    pub fn new() -> Self {
        Self {
            intervals: BTreeSet::<Interval>::new(),
        }
    }

    pub fn insert(&mut self, interval: Interval) {
        let left_interval = Interval::point(interval.start - 1);
        let right_interval = Interval::point(interval.end + 1);
        let left = self.intervals.get(&left_interval);
        let right = self.intervals.get(&right_interval);

        let interval = if let (Some(l), Some(r)) = (left, right) {
            let new_interval = interval.envelope(l, r);
            self.intervals.remove(&left_interval);
            self.intervals.remove(&right_interval);
            new_interval
        } else if let Some(l) = left {
            let new_interval = interval.envelope(l, l);
            self.intervals.remove(&left_interval);
            new_interval
        } else if let Some(r) = right {
            let new_interval = interval.envelope(r, r);
            self.intervals.remove(&right_interval);
            new_interval
        } else {
            interval
        };
        let inserted = self.intervals.insert(interval);
        assert!(inserted);
    }

    pub fn get(&self, value: &Interval) -> Option<&Interval> {
        self.intervals.get(value)
    }

    pub fn first(&self) -> Option<&Interval> {
        self.intervals.first()
    }

    pub fn last(&self) -> Option<&Interval> {
        self.intervals.last()
    }

    pub fn len(&self) -> usize {
        self.intervals.len()
    }

    pub fn is_empty(&self) -> bool {
        self.intervals.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_interval_set() {
        let mut intervals = IntervalSet::new();

        // {[1..1]}
        intervals.insert(Interval::point(1));
        assert_eq!(intervals.len(), 1);
        assert_eq!(intervals.last(), Some(Interval::point(1)).as_ref());
        assert_eq!(intervals.first(), Some(Interval::point(1)).as_ref());

        // {[1..1], [3..3]}
        intervals.insert(Interval::point(3));
        assert_eq!(intervals.len(), 2);
        assert_eq!(intervals.last(), Some(Interval::point(1)).as_ref());
        assert_eq!(intervals.first(), Some(Interval::point(3)).as_ref());

        // {[1..1], [3..4]}
        intervals.insert(Interval::point(4));
        assert_eq!(intervals.len(), 2);
        assert_eq!(intervals.last(), Some(Interval::point(1)).as_ref());
        assert_eq!(intervals.first(), Some(Interval::new(3, 4)).as_ref());

        // {[1..1], [3..4], [7..7]}
        intervals.insert(Interval::point(7));
        assert_eq!(intervals.len(), 3);
        assert_eq!(intervals.last(), Some(Interval::point(1)).as_ref());
        assert_eq!(intervals.first(), Some(Interval::point(7)).as_ref());

        // {[1..1], [3..4], [6..7]}
        intervals.insert(Interval::point(6));
        assert_eq!(intervals.len(), 3);
        assert_eq!(intervals.last(), Some(Interval::point(1)).as_ref());
        assert_eq!(
            intervals.get(&Interval::point(3)),
            Some(Interval::new(3, 4)).as_ref()
        );
        assert_eq!(
            intervals.get(&Interval::point(4)),
            Some(Interval::new(3, 4)).as_ref()
        );
        assert_eq!(
            intervals.get(&Interval::new(3, 4)),
            Some(Interval::new(3, 4)).as_ref()
        );
        assert_eq!(
            intervals.get(&Interval::new(4, 5)),
            Some(Interval::new(3, 4)).as_ref()
        );
        assert_eq!(
            intervals.get(&Interval::new(2, 3)),
            Some(Interval::new(3, 4)).as_ref()
        );
        assert_eq!(
            intervals.get(&Interval::new(2, 5)),
            Some(Interval::new(3, 4)).as_ref()
        );
        assert_eq!(intervals.get(&Interval::point(2)), None);
        assert_eq!(intervals.get(&Interval::point(5)), None);
        assert_eq!(
            intervals.get(&Interval::new(3, 6)),
            Some(Interval::new(6, 7)).as_ref()
        );
        assert_eq!(
            intervals.get(&Interval::new(4, 6)),
            Some(Interval::new(6, 7)).as_ref()
        );
        assert_eq!(
            intervals.get(&Interval::new(1, 7)),
            Some(Interval::new(6, 7)).as_ref()
        );
        assert_eq!(intervals.first(), Some(Interval::new(6, 7)).as_ref());

        // {[1..4], [6..7]}
        intervals.insert(Interval::point(2));
        assert_eq!(intervals.len(), 2);
        assert_eq!(intervals.last(), Some(Interval::new(1, 4)).as_ref());
        assert_eq!(intervals.first(), Some(Interval::new(6, 7)).as_ref());

        // {[1..7]}
        intervals.insert(Interval::point(5));
        assert_eq!(intervals.len(), 1);
        assert_eq!(intervals.last(), Some(Interval::new(1, 7)).as_ref());
        assert_eq!(intervals.first(), Some(Interval::new(1, 7)).as_ref());
    }
}
