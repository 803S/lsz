use crate::{
    domain::bookmarks::BookmarkRecord, domain::entry::EntryItem, domain::preview::PreviewModel,
    surfaces::tui::app::state::SortMode,
};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum InputEdit {
    Insert(char),
    Backspace,
}

#[derive(Debug, Clone)]
pub enum Action {
    LoadDir(PathBuf),
    DirLoaded {
        cwd: PathBuf,
        entries: Vec<EntryItem>,
    },
    DirFailed(String),
    MoveDown,
    MoveUp,
    OpenSelected,
    GoParent,
    Refresh,
    ToggleHidden,
    FocusNext,
    OpenSearch,
    SearchInput(InputEdit),
    SubmitSearch,
    ClearSearch,
    OpenCommand,
    CommandInput(InputEdit),
    SubmitCommand,
    SetSort(SortMode),
    OpenHelp,
    CloseOverlay,
    HelpInput(InputEdit),
    HelpScroll(i16),
    HelpNextCategory,
    HelpPrevCategory,
    OpenNoteEditor,
    NoteInput(InputEdit),
    NoteClearLine,
    SaveNote,
    ToggleBookmark,
    OpenBookmarkPicker,
    BookmarksLoaded(Vec<BookmarkRecord>),
    BookmarkNext,
    BookmarkPrev,
    ConfirmBookmark,
    DeleteBookmark,
    ConfirmAccept,
    RequestPreview,
    OpenReader,
    PreviewLoaded {
        path: PathBuf,
        preview: PreviewModel,
    },
    ScrollPreview(i16),
    ScrollPreviewHorizontal(i16),
    ToggleLineNumbers,
    FocusExplorer,
    OpenExternal,
    Status(String),
    Quit,
}
