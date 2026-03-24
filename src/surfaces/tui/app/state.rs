use crate::{
    config::{
        keymap::{category_label, context_label, default_key_bindings},
        theme::AppTheme,
    },
    domain::{
        bookmarks::BookmarkRecord,
        entry::EntryItem,
        keymap::{HelpCategory, HelpContext, KeyBinding},
        preview::PreviewModel,
    },
    infra::cache::PreviewCache,
};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPane {
    Explorer,
    Preview,
    Inspector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
    Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    Name,
    Mtime,
}

#[derive(Debug, Clone)]
pub enum ConfirmIntent {
    DeleteBookmark { name: String },
    ClearNote { path: PathBuf },
}

#[derive(Debug, Clone)]
pub enum OverlayState {
    None,
    Help {
        filter: String,
        scroll: u16,
        context: HelpContext,
        category: HelpCategory,
    },
    NoteEditor {
        path: PathBuf,
        input: String,
        original: Option<String>,
    },
    Confirm {
        title: String,
        message: String,
        intent: ConfirmIntent,
        return_to: Option<Box<OverlayState>>,
    },
    BookmarkPicker {
        items: Vec<BookmarkRecord>,
        selected: usize,
    },
    Reader,
}

pub struct AppState {
    pub cwd: PathBuf,
    pub include_hidden: bool,
    pub sort_mode: SortMode,
    pub focus: FocusPane,
    pub input_mode: InputMode,
    pub overlay: OverlayState,
    pub entries: Vec<EntryItem>,
    pub filtered: Vec<usize>,
    pub selected: Option<usize>,
    pub search_input: String,
    pub command_input: String,
    pub preview: PreviewModel,
    pub preview_target: Option<PathBuf>,
    pub preview_scroll: u16,
    pub preview_scroll_x: u16,
    pub status: String,
    pub theme: AppTheme,
    pub key_bindings: Vec<KeyBinding>,
    pub preview_cache: PreviewCache,
    pub initial_selection: Option<PathBuf>,
    pub show_line_numbers: bool,
    pub help_resume_overlay: Option<Box<OverlayState>>,
    pub help_resume_input_mode: Option<InputMode>,
    pub help_resume_focus: Option<FocusPane>,
}

impl AppState {
    pub fn new(cwd: PathBuf, initial_selection: Option<PathBuf>) -> Self {
        Self {
            cwd: cwd.clone(),
            include_hidden: false,
            sort_mode: SortMode::Name,
            focus: FocusPane::Explorer,
            input_mode: InputMode::Normal,
            overlay: OverlayState::None,
            entries: Vec::new(),
            filtered: Vec::new(),
            selected: None,
            search_input: String::new(),
            command_input: String::new(),
            preview: PreviewModel::Loading { path: cwd.clone() },
            preview_target: None,
            preview_scroll: 0,
            preview_scroll_x: 0,
            status: String::new(),
            theme: AppTheme::default(),
            key_bindings: default_key_bindings(),
            preview_cache: PreviewCache::new(5),
            initial_selection,
            show_line_numbers: true,
            help_resume_overlay: None,
            help_resume_input_mode: None,
            help_resume_focus: None,
        }
    }

    pub fn apply_filter(&mut self) {
        if self.search_input.is_empty() {
            self.filtered = (0..self.entries.len()).collect();
        } else {
            let query = self.search_input.to_lowercase();
            self.filtered = self
                .entries
                .iter()
                .enumerate()
                .filter(|(_, entry)| {
                    entry.name.to_lowercase().contains(&query)
                        || entry
                            .path
                            .display()
                            .to_string()
                            .to_lowercase()
                            .contains(&query)
                        || entry
                            .note
                            .as_ref()
                            .is_some_and(|note| note.to_lowercase().contains(&query))
                        || entry
                            .auto_summary
                            .as_ref()
                            .is_some_and(|summary| summary.to_lowercase().contains(&query))
                })
                .map(|(idx, _)| idx)
                .collect();
        }
        self.selected = if self.filtered.is_empty() {
            None
        } else {
            Some(0)
        };
        self.reset_preview_position();
    }

    pub fn set_entries(&mut self, entries: Vec<EntryItem>) {
        self.entries = entries;
        self.sort_entries();
        self.apply_filter();
        if let Some(target) = self.initial_selection.clone() {
            if self.select_path(&target) {
                self.initial_selection = None;
            }
        }
    }

    pub fn visible_len(&self) -> usize {
        self.filtered.len()
    }

    pub fn selected_entry(&self) -> Option<&EntryItem> {
        let selected = self.selected?;
        let idx = *self.filtered.get(selected)?;
        self.entries.get(idx)
    }

    pub fn selected_entry_path(&self) -> Option<&Path> {
        self.selected_entry().map(|entry| entry.path.as_path())
    }

    pub fn selected_entry_clone(&self) -> Option<EntryItem> {
        self.selected_entry().cloned()
    }

    pub fn move_next(&mut self) {
        let len = self.visible_len();
        if len == 0 {
            self.selected = None;
            return;
        }
        self.selected = Some(match self.selected {
            Some(index) if index + 1 < len => index + 1,
            _ => 0,
        });
        self.reset_preview_position();
    }

    pub fn move_prev(&mut self) {
        let len = self.visible_len();
        if len == 0 {
            self.selected = None;
            return;
        }
        self.selected = Some(match self.selected {
            Some(index) if index > 0 => index - 1,
            _ => len.saturating_sub(1),
        });
        self.reset_preview_position();
    }

    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status = status.into();
    }

    pub fn reset_preview_position(&mut self) {
        self.preview_scroll = 0;
        self.preview_scroll_x = 0;
    }

    pub fn set_sort_mode(&mut self, sort_mode: SortMode) {
        self.sort_mode = sort_mode;
        self.sort_entries();
        self.apply_filter();
    }

    pub fn sort_mode_label(&self) -> &'static str {
        match self.sort_mode {
            SortMode::Name => "名称",
            SortMode::Mtime => "时间",
        }
    }

    pub fn current_help_context(&self) -> HelpContext {
        match &self.overlay {
            OverlayState::Help { context, .. } => *context,
            OverlayState::BookmarkPicker { .. } => HelpContext::BookmarkPicker,
            OverlayState::NoteEditor { .. } => HelpContext::OverlayNoteEditor,
            OverlayState::Confirm { .. } => HelpContext::OverlayConfirm,
            OverlayState::Reader => HelpContext::Reader,
            OverlayState::None => match self.input_mode {
                InputMode::Command => HelpContext::CommandLine,
                InputMode::Normal | InputMode::Search => match self.focus {
                    FocusPane::Explorer => HelpContext::Explorer,
                    FocusPane::Preview => HelpContext::Preview,
                    FocusPane::Inspector => HelpContext::Inspector,
                },
            },
        }
    }

    pub fn help_categories_for(&self, context: HelpContext) -> Vec<HelpCategory> {
        let mut categories = self
            .key_bindings
            .iter()
            .filter(|binding| binding.context == context)
            .map(|binding| binding.category)
            .collect::<Vec<_>>();
        categories.sort();
        categories.dedup();
        if categories.is_empty() {
            categories.push(HelpCategory::Advanced);
        }
        categories
    }

    pub fn footer_bindings(&self) -> Vec<KeyBinding> {
        let context = self.current_help_context();
        let mut bindings = Vec::new();
        if self.input_mode == InputMode::Normal && !self.search_input.is_empty() {
            bindings.push(KeyBinding::primary(
                context,
                HelpCategory::SearchFilter,
                &["Esc"],
                "清滤",
                "清除当前过滤",
            ));
        }
        bindings.extend(
            self.key_bindings
                .iter()
                .filter(|binding| binding.primary && binding.context == context)
                .cloned(),
        );
        bindings.truncate(6);
        bindings
    }

    pub fn help_bindings(&self) -> Vec<KeyBinding> {
        let (filter, context, category) = match &self.overlay {
            OverlayState::Help {
                filter,
                context,
                category,
                ..
            } => (filter.to_lowercase(), *context, *category),
            _ => return Vec::new(),
        };
        let search_all_categories = !filter.trim().is_empty();
        let mut bindings = self
            .key_bindings
            .iter()
            .filter(|binding| binding.context == context)
            .filter(|binding| search_all_categories || binding.category == category)
            .filter(|binding| help_binding_matches(binding, &filter))
            .cloned()
            .collect::<Vec<_>>();
        bindings.sort_by_key(|binding| (!binding.primary, binding.category, binding.label.clone()));
        bindings
    }

    pub fn rotate_help_category(&mut self, forward: bool) {
        let (context, current) = match &self.overlay {
            OverlayState::Help {
                context, category, ..
            } => (*context, *category),
            _ => return,
        };
        let categories = self.help_categories_for(context);
        if categories.is_empty() {
            return;
        }
        let current_index = categories
            .iter()
            .position(|item| *item == current)
            .unwrap_or(0);
        let next_index = if forward {
            (current_index + 1) % categories.len()
        } else if current_index == 0 {
            categories.len().saturating_sub(1)
        } else {
            current_index - 1
        };
        if let OverlayState::Help {
            category, scroll, ..
        } = &mut self.overlay
        {
            *category = categories[next_index];
            *scroll = 0;
        }
    }

    pub fn begin_help_overlay(&mut self) {
        let context = self.current_help_context();
        self.help_resume_overlay = match &self.overlay {
            OverlayState::None => None,
            current => Some(Box::new(current.clone())),
        };
        self.help_resume_input_mode = Some(self.input_mode);
        self.help_resume_focus = Some(self.focus);
        let category = self
            .help_categories_for(context)
            .into_iter()
            .next()
            .unwrap_or(HelpCategory::Advanced);
        self.overlay = OverlayState::Help {
            filter: String::new(),
            scroll: 0,
            context,
            category,
        };
        self.input_mode = InputMode::Normal;
    }

    pub fn end_help_overlay(&mut self) {
        if !matches!(self.overlay, OverlayState::Help { .. }) {
            self.overlay = OverlayState::None;
            self.input_mode = InputMode::Normal;
            return;
        }
        self.overlay = self
            .help_resume_overlay
            .take()
            .map(|overlay| *overlay)
            .unwrap_or(OverlayState::None);
        self.input_mode = self
            .help_resume_input_mode
            .take()
            .unwrap_or(InputMode::Normal);
        self.focus = self.help_resume_focus.take().unwrap_or(FocusPane::Explorer);
    }

    fn sort_entries(&mut self) {
        self.entries.sort_by(|a, b| {
            let directory_order = match (&a.kind, &b.kind) {
                (
                    crate::domain::entry::EntryKind::Directory,
                    crate::domain::entry::EntryKind::Directory,
                ) => std::cmp::Ordering::Equal,
                (crate::domain::entry::EntryKind::Directory, _) => std::cmp::Ordering::Less,
                (_, crate::domain::entry::EntryKind::Directory) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            };
            if directory_order != std::cmp::Ordering::Equal {
                return directory_order;
            }
            match self.sort_mode {
                SortMode::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortMode::Mtime => b
                    .mtime
                    .cmp(&a.mtime)
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
            }
        });
    }

    fn select_path(&mut self, path: &Path) -> bool {
        let Some(index) = self.filtered.iter().position(|entry_idx| {
            self.entries
                .get(*entry_idx)
                .is_some_and(|entry| entry.path == path)
        }) else {
            return false;
        };
        self.selected = Some(index);
        true
    }
}

fn help_binding_matches(binding: &KeyBinding, filter: &str) -> bool {
    let needle = normalize_help_query(filter);
    if needle.is_empty() {
        return true;
    }

    let haystacks = [
        binding.keys.join(" "),
        binding.label.clone(),
        binding.detail.clone(),
        context_label(binding.context).to_string(),
        category_label(binding.category).to_string(),
        help_search_alias(binding),
    ];

    haystacks.into_iter().any(|haystack| {
        let haystack = normalize_help_query(&haystack);
        haystack.contains(&needle) || fuzzy_contains(&haystack, &needle)
    })
}

fn normalize_help_query(text: &str) -> String {
    text.chars()
        .flat_map(|ch| ch.to_lowercase())
        .filter(|ch| {
            ch.is_ascii_alphanumeric() || (!ch.is_ascii_punctuation() && !ch.is_whitespace())
        })
        .collect()
}

fn fuzzy_contains(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let mut needle_chars = needle.chars();
    let mut current = needle_chars.next();
    for ch in haystack.chars() {
        if Some(ch) == current {
            current = needle_chars.next();
            if current.is_none() {
                return true;
            }
        }
    }
    false
}

fn help_search_alias(binding: &KeyBinding) -> String {
    let mut aliases = Vec::new();
    for text in [
        binding.label.as_str(),
        binding.detail.as_str(),
        context_label(binding.context),
        category_label(binding.category),
    ] {
        aliases.extend(text_search_aliases(text));
    }
    aliases.join(" ")
}

fn text_search_aliases(text: &str) -> Vec<&'static str> {
    const ALIASES: &[(&str, &[&str])] = &[
        ("浏览", &["liulan", "ll"]),
        ("预览", &["yulan", "yl"]),
        ("信息", &["xinxi", "xx"]),
        ("阅读", &["yuedu", "yd"]),
        ("帮助", &["bangzhu", "bz"]),
        ("备注", &["beizhu", "bz"]),
        ("书签", &["shuqian", "sq"]),
        ("收藏", &["shoucang", "sc"]),
        ("列表", &["liebiao", "lb"]),
        ("搜索", &["sousuo", "ss"]),
        ("筛选", &["shaixuan", "sx"]),
        ("过滤", &["guolv", "gl"]),
        ("关键字", &["guanjianzi", "gjz"]),
        ("打开", &["dakai", "dk"]),
        ("关闭", &["guanbi", "gb"]),
        ("返回", &["fanhui", "fh"]),
        ("滚动", &["gundong", "gd"]),
        ("翻页", &["fanye", "fy"]),
        ("横移", &["hengyi", "hy"]),
        ("切换", &["qiehuan", "qh"]),
        ("焦点", &["jiaodian", "jd"]),
        ("命令", &["mingling", "ml"]),
        ("排序", &["paixu", "px"]),
        ("时间", &["shijian", "sj"]),
        ("名称", &["mingcheng", "mc"]),
        ("删除", &["shanchu", "sc"]),
        ("确认", &["queren", "qr"]),
        ("取消", &["quxiao", "qx"]),
        ("外部打开", &["waibudakai", "wbdk"]),
        ("行号", &["hanghao", "hh"]),
        ("隐藏文件", &["yincangwenjian", "ycwj"]),
        ("目录", &["mulu", "ml"]),
        ("文件", &["wenjian", "wj"]),
        ("编辑", &["bianji", "bj"]),
        ("跳转", &["tiaozhuan", "tz"]),
        ("选择", &["xuanze", "xz"]),
        ("高级动作", &["gaojidongzuo", "gjdz"]),
        ("基础导航", &["jichudaohang", "jcdh"]),
        ("搜索与筛选", &["sousuoyushaixuan", "ssysx"]),
        ("备注与书签", &["beizhuyushuqian", "bzysq"]),
        ("布局与焦点", &["bujuyujiaodian", "bjyjd"]),
        ("预览与动作", &["yulanyudongzuo", "ylydz"]),
    ];

    let mut aliases = Vec::new();
    for (needle, values) in ALIASES {
        if text.contains(needle) {
            aliases.extend_from_slice(values);
        }
    }
    aliases
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entry::{EntryItem, EntryKind};
    use std::time::{Duration, SystemTime};

    fn entry(name: &str, kind: EntryKind, modified_after_epoch: u64) -> EntryItem {
        EntryItem {
            path: PathBuf::from(format!("/tmp/{name}")),
            name: name.to_string(),
            kind,
            is_hidden: false,
            is_symlink: false,
            size: Some(1),
            mtime: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(modified_after_epoch)),
            note: None,
            auto_summary: None,
        }
    }

    #[test]
    fn sort_mode_mtime_keeps_directories_first() {
        let cwd = PathBuf::from("/tmp");
        let mut state = AppState::new(cwd, None);
        state.set_entries(vec![
            entry("b.txt", EntryKind::Text, 1),
            entry("a.txt", EntryKind::Text, 10),
            entry("dir", EntryKind::Directory, 2),
        ]);

        state.set_sort_mode(SortMode::Mtime);

        let visible = state
            .filtered
            .iter()
            .filter_map(|idx| state.entries.get(*idx))
            .map(|entry| entry.name.clone())
            .collect::<Vec<_>>();
        assert_eq!(visible, vec!["dir", "a.txt", "b.txt"]);
    }

    #[test]
    fn footer_only_shows_limited_primary_bindings() {
        let state = AppState::new(PathBuf::from("/tmp"), None);
        assert!(state.footer_bindings().len() <= 6);
    }

    #[test]
    fn help_filter_searches_all_categories_and_supports_pinyin() {
        let mut state = AppState::new(PathBuf::from("/tmp"), None);
        state.begin_help_overlay();
        if let OverlayState::Help { filter, .. } = &mut state.overlay {
            *filter = "sqlb".to_string();
        }

        let bindings = state.help_bindings();
        assert!(bindings.iter().any(|binding| binding.label == "书签列表"));
    }
}
