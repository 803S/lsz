use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct BookmarkRecord {
    pub name: String,
    pub path: PathBuf,
    pub note: Option<String>,
}
