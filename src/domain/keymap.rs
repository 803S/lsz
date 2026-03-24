#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpContext {
    Explorer,
    Preview,
    Inspector,
    Reader,
    OverlayHelp,
    OverlayNoteEditor,
    OverlayConfirm,
    CommandLine,
    BookmarkPicker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HelpCategory {
    Navigation,
    OpenClose,
    SearchFilter,
    NotesBookmarks,
    PreviewAction,
    LayoutFocus,
    Editing,
    Advanced,
}

#[derive(Debug, Clone)]
pub struct KeyBinding {
    pub context: HelpContext,
    pub category: HelpCategory,
    pub keys: Vec<String>,
    pub label: String,
    pub detail: String,
    pub primary: bool,
}

impl KeyBinding {
    pub fn new(
        context: HelpContext,
        category: HelpCategory,
        keys: &[&str],
        label: &str,
        detail: &str,
    ) -> Self {
        Self {
            context,
            category,
            keys: keys.iter().map(|key| (*key).to_string()).collect(),
            label: label.to_string(),
            detail: detail.to_string(),
            primary: false,
        }
    }

    pub fn primary(
        context: HelpContext,
        category: HelpCategory,
        keys: &[&str],
        label: &str,
        detail: &str,
    ) -> Self {
        let mut binding = Self::new(context, category, keys, label, detail);
        binding.primary = true;
        binding
    }
}
