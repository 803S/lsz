use crate::{
    domain::bookmarks::BookmarkRecord, domain::entry::EntryItem, domain::preview::PreviewModel,
};
use std::path::PathBuf;

#[derive(Debug)]
pub enum AppEvent {
    DirLoaded {
        cwd: PathBuf,
        entries: Vec<EntryItem>,
    },
    DirFailed(String),
    PreviewLoaded {
        path: PathBuf,
        preview: PreviewModel,
    },
    BookmarksLoaded(Vec<BookmarkRecord>),
    Status(String),
}
