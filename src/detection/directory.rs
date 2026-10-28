use std::fs::DirEntry;
use std::path::Path;

pub(crate) struct DirectoryListing {
    entries: Vec<DirEntry>,
    unreadable: bool,
}

impl DirectoryListing {
    pub(crate) fn read(path: &Path) -> Self {
        let mut listing = Self {
            entries: Vec::new(),
            unreadable: false,
        };
        match std::fs::read_dir(path) {
            Ok(entries) => {
                for entry in entries {
                    match entry {
                        Ok(entry) => listing.entries.push(entry),
                        Err(_) => listing.unreadable = true,
                    }
                }
            }
            Err(_) => listing.unreadable = true,
        }
        listing
    }

    pub(crate) fn entries(&self) -> impl Iterator<Item = &DirEntry> {
        self.entries.iter()
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn unreadable(&self) -> bool {
        self.unreadable
    }
}
