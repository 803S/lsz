use std::{path::PathBuf, time::SystemTime};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryKind {
    Directory,
    Markdown,
    Code,
    Text,
    Archive,
    Pdf,
    Image,
    Binary,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct EntryItem {
    pub path: PathBuf,
    pub name: String,
    pub kind: EntryKind,
    pub is_hidden: bool,
    pub is_symlink: bool,
    pub size: Option<u64>,
    pub mtime: Option<SystemTime>,
    pub note: Option<String>,
    pub auto_summary: Option<String>,
}

impl EntryItem {
    pub fn display_name(&self) -> String {
        if matches!(self.kind, EntryKind::Directory) {
            format!("{}/", self.name)
        } else {
            self.name.clone()
        }
    }
}
