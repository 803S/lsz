use std::path::PathBuf;

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone)]
pub struct NoteRecord {
    pub path: PathBuf,
    pub note: String,
}
