use std::{
    cmp::{Ordering, Reverse},
    fmt,
    path::{Path, PathBuf},
};

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
    pub fn new(path: &Path) -> Option<Self> {
        let Some(extension) = path.extension() else {return None;};
        let Some(priority) = extension.to_str()?.parse::<i32>().ok() else {return None;};
        Some(Fragment {
            priority: Reverse(priority),
            path: path.to_path_buf(),
        })
    }
}
