use crate::{
    domain::entry::EntryKind,
    domain::preview::PreviewModel,
    surfaces::tui::app::{
        action::{Action, InputEdit},
        effect::Effect,
        state::{AppState, ConfirmIntent, FocusPane, InputMode, OverlayState, SortMode},
    },
};
use std::path::PathBuf;

pub struct ReduceResult {
    pub effects: Vec<Effect>,
    pub should_quit: bool,
}

impl ReduceResult {
    fn none() -> Self {
        Self {
            effects: Vec::new(),
            should_quit: false,
        }
    }
}

pub fn reduce(state: &mut AppState, action: Action) -> ReduceResult {
    match action {
        Action::LoadDir(cwd) => {
            state.cwd = cwd.clone();
            state.preview = PreviewModel::Loading { path: cwd.clone() };
            state.set_status(format!("正在加载 {}", cwd.display()));
            ReduceResult {
                effects: vec![Effect::LoadDir {
                    cwd,
                    include_hidden: state.include_hidden,
                }],
                should_quit: false,
            }
        }
        Action::DirLoaded { cwd, entries } => {
            state.cwd = cwd;
            state.set_entries(entries);
            state.set_status(format!("已加载 {}", state.cwd.display()));
            reduce(state, Action::RequestPreview)
        }
        Action::DirFailed(message) => {
            state.set_status(message);
            ReduceResult::none()
        }
        Action::MoveDown => {
            state.move_next();
            reduce(state, Action::RequestPreview)
        }
        Action::MoveUp => {
            state.move_prev();
            reduce(state, Action::RequestPreview)
        }
        Action::OpenSelected => {
            let Some(entry) = state.selected_entry_clone() else {
                return ReduceResult::none();
            };
            match entry.kind {
                EntryKind::Directory => reduce(state, Action::LoadDir(entry.path)),
                _ => reduce(state, Action::OpenReader),
            }
        }
        Action::GoParent => {
            if let Some(parent) = state.cwd.parent() {
                return reduce(state, Action::LoadDir(parent.to_path_buf()));
            }
            ReduceResult::none()
        }
        Action::Refresh => reduce(state, Action::LoadDir(state.cwd.clone())),
        Action::ToggleHidden => {
            state.include_hidden = !state.include_hidden;
            reduce(state, Action::LoadDir(state.cwd.clone()))
        }
        Action::FocusNext => {
            state.focus = match state.focus {
                FocusPane::Explorer => FocusPane::Preview,
                FocusPane::Preview => FocusPane::Inspector,
                FocusPane::Inspector => FocusPane::Explorer,
            };
            ReduceResult::none()
        }
        Action::OpenSearch => {
            state.input_mode = InputMode::Search;
            state.set_status(if state.search_input.is_empty() {
                "搜索模式"
            } else {
                "搜索模式：可继续修改当前过滤"
            });
            ReduceResult::none()
        }
        Action::SearchInput(edit) => {
            match edit {
                InputEdit::Insert(ch) => state.search_input.push(ch),
                InputEdit::Backspace => {
                    state.search_input.pop();
                }
            }
            state.apply_filter();
            reduce(state, Action::RequestPreview)
        }
        Action::SubmitSearch => {
            state.input_mode = InputMode::Normal;
            if state.search_input.trim().is_empty() {
                state.set_status("搜索已清空");
            } else {
                state.set_status(format!(
                    "已应用搜索: {}（{} 项）",
                    state.search_input,
                    state.visible_len()
                ));
            }
            reduce(state, Action::RequestPreview)
        }
        Action::ClearSearch => {
            let had_filter = !state.search_input.is_empty();
            state.input_mode = InputMode::Normal;
            state.search_input.clear();
            state.apply_filter();
            if had_filter {
                state.set_status("已清除过滤");
            }
            reduce(state, Action::RequestPreview)
        }
        Action::OpenCommand => {
            state.input_mode = InputMode::Command;
            state.command_input.clear();
            state.set_status("命令模式：help / sort name / bookmark add [name] / note clear");
            ReduceResult::none()
        }
        Action::CommandInput(edit) => {
            match edit {
                InputEdit::Insert(ch) => state.command_input.push(ch),
                InputEdit::Backspace => {
                    state.command_input.pop();
                }
            }
            ReduceResult::none()
        }
        Action::SubmitCommand => {
            let command = state.command_input.trim().to_string();
            state.command_input.clear();
            state.input_mode = InputMode::Normal;
            handle_command(state, &command)
        }
        Action::SetSort(sort_mode) => {
            state.set_sort_mode(sort_mode);
            state.set_status(format!("排序: {}", state.sort_mode_label()));
            reduce(state, Action::RequestPreview)
        }
        Action::OpenHelp => {
            state.begin_help_overlay();
            ReduceResult::none()
        }
        Action::CloseOverlay => {
            match &state.overlay {
                OverlayState::Help { .. } => state.end_help_overlay(),
                OverlayState::Confirm { return_to, .. } => {
                    state.overlay = return_to
                        .clone()
                        .map(|overlay| *overlay)
                        .unwrap_or(OverlayState::None);
                    state.input_mode = InputMode::Normal;
                }
                _ => {
                    state.overlay = OverlayState::None;
                    state.input_mode = InputMode::Normal;
                }
            }
            ReduceResult::none()
        }
        Action::HelpInput(edit) => {
            if let OverlayState::Help { filter, scroll, .. } = &mut state.overlay {
                match edit {
                    InputEdit::Insert(ch) => filter.push(ch),
                    InputEdit::Backspace => {
                        filter.pop();
                    }
                }
                *scroll = 0;
            }
            ReduceResult::none()
        }
        Action::HelpNextCategory => {
            state.rotate_help_category(true);
            ReduceResult::none()
        }
        Action::HelpPrevCategory => {
            state.rotate_help_category(false);
            ReduceResult::none()
        }
        Action::HelpScroll(delta) => {
            if let OverlayState::Help { scroll, .. } = &mut state.overlay {
                if delta >= 0 {
                    *scroll = scroll.saturating_add(delta as u16);
                } else {
                    *scroll = scroll.saturating_sub(delta.unsigned_abs());
                }
            }
            ReduceResult::none()
        }
        Action::OpenNoteEditor => {
            let Some(entry) = state.selected_entry_clone() else {
                return ReduceResult::none();
            };
            state.overlay = OverlayState::NoteEditor {
                path: entry.path,
                input: entry.note.clone().unwrap_or_default(),
                original: entry.note,
            };
            ReduceResult::none()
        }
        Action::NoteInput(edit) => {
            if let OverlayState::NoteEditor { input, .. } = &mut state.overlay {
                match edit {
                    InputEdit::Insert(ch) => input.push(ch),
                    InputEdit::Backspace => {
                        input.pop();
                    }
                }
            }
            ReduceResult::none()
        }
        Action::NoteClearLine => {
            if let OverlayState::NoteEditor { input, .. } = &mut state.overlay {
                input.clear();
            }
            ReduceResult::none()
        }
        Action::SaveNote => {
            let overlay = std::mem::replace(&mut state.overlay, OverlayState::None);
            if let OverlayState::NoteEditor {
                path,
                input,
                original,
            } = overlay
            {
                state.set_status(format!("正在保存备注: {}", path.display()));
                let effect = if input.trim().is_empty() && original.is_some() {
                    Effect::DeleteNote { path }
                } else {
                    Effect::SaveNote { path, note: input }
                };
                ReduceResult {
                    effects: vec![
                        effect,
                        Effect::LoadDir {
                            cwd: state.cwd.clone(),
                            include_hidden: state.include_hidden,
                        },
                    ],
                    should_quit: false,
                }
            } else {
                ReduceResult::none()
            }
        }
        Action::ToggleBookmark => {
            let path = state
                .selected_entry()
                .filter(|entry| matches!(entry.kind, EntryKind::Directory))
                .map(|entry| entry.path.clone())
                .unwrap_or_else(|| state.cwd.clone());
            let name = path
                .file_name()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_else(|| "根目录".to_string());
            ReduceResult {
                effects: vec![Effect::ToggleBookmark { name, path }],
                should_quit: false,
            }
        }
        Action::OpenBookmarkPicker => ReduceResult {
            effects: vec![Effect::LoadBookmarks],
            should_quit: false,
        },
        Action::BookmarksLoaded(bookmarks) => {
            state.overlay = OverlayState::BookmarkPicker {
                items: bookmarks,
                selected: 0,
            };
            ReduceResult::none()
        }
        Action::BookmarkNext => {
            if let OverlayState::BookmarkPicker { items, selected } = &mut state.overlay {
                if !items.is_empty() {
                    *selected = (*selected + 1).min(items.len().saturating_sub(1));
                }
            }
            ReduceResult::none()
        }
        Action::BookmarkPrev => {
            if let OverlayState::BookmarkPicker { selected, .. } = &mut state.overlay {
                *selected = selected.saturating_sub(1);
            }
            ReduceResult::none()
        }
        Action::ConfirmBookmark => {
            let target = if let OverlayState::BookmarkPicker { items, selected } = &state.overlay {
                items.get(*selected).map(|bookmark| bookmark.path.clone())
            } else {
                None
            };
            if let Some(path) = target {
                state.overlay = OverlayState::None;
                return reduce(state, Action::LoadDir(path));
            }
            ReduceResult::none()
        }
        Action::DeleteBookmark => {
            let target = if let OverlayState::BookmarkPicker { items, selected } = &state.overlay {
                items.get(*selected).map(|bookmark| bookmark.name.clone())
            } else {
                None
            };
            if let Some(name) = target {
                state.overlay = OverlayState::Confirm {
                    title: "删除书签".to_string(),
                    message: format!("确定删除书签“{name}”吗？"),
                    intent: ConfirmIntent::DeleteBookmark { name },
                    return_to: Some(Box::new(state.overlay.clone())),
                };
            }
            ReduceResult::none()
        }
        Action::ConfirmAccept => {
            let overlay = std::mem::replace(&mut state.overlay, OverlayState::None);
            if let OverlayState::Confirm {
                intent, return_to, ..
            } = overlay
            {
                state.overlay = return_to
                    .map(|overlay| *overlay)
                    .unwrap_or(OverlayState::None);
                match intent {
                    ConfirmIntent::DeleteBookmark { name } => ReduceResult {
                        effects: vec![Effect::DeleteBookmark { name }],
                        should_quit: false,
                    },
                    ConfirmIntent::ClearNote { path } => ReduceResult {
                        effects: vec![
                            Effect::DeleteNote { path },
                            Effect::LoadDir {
                                cwd: state.cwd.clone(),
                                include_hidden: state.include_hidden,
                            },
                        ],
                        should_quit: false,
                    },
                }
            } else {
                ReduceResult::none()
            }
        }
        Action::RequestPreview => {
            let Some(entry) = state.selected_entry_clone() else {
                state.preview = PreviewModel::Loading {
                    path: state.cwd.clone(),
                };
                state.reset_preview_position();
                return ReduceResult::none();
            };
            state.reset_preview_position();
            if let Some(cached) = state.preview_cache.get(&entry.path) {
                state.preview_target = Some(entry.path.clone());
                state.preview = cached;
                return ReduceResult::none();
            }
            state.preview_target = Some(entry.path.clone());
            state.preview = PreviewModel::Loading {
                path: entry.path.clone(),
            };
            ReduceResult {
                effects: vec![Effect::LoadPreview {
                    entry,
                    viewport_hint: 220,
                }],
                should_quit: false,
            }
        }
        Action::OpenReader => {
            if state.selected_entry().is_some() {
                state.overlay = OverlayState::Reader;
            }
            ReduceResult::none()
        }
        Action::PreviewLoaded { path, preview } => {
            state.preview_cache.put(path.clone(), preview.clone());
            if state.preview_target.as_ref() == Some(&path) {
                state.preview = preview;
            }
            ReduceResult::none()
        }
        Action::ScrollPreview(delta) => {
            if delta >= 0 {
                state.preview_scroll = state.preview_scroll.saturating_add(delta as u16);
            } else {
                state.preview_scroll = state.preview_scroll.saturating_sub(delta.unsigned_abs());
            }
            ReduceResult::none()
        }
        Action::ScrollPreviewHorizontal(delta) => {
            if delta >= 0 {
                state.preview_scroll_x = state.preview_scroll_x.saturating_add(delta as u16);
            } else {
                state.preview_scroll_x =
                    state.preview_scroll_x.saturating_sub(delta.unsigned_abs());
            }
            ReduceResult::none()
        }
        Action::ToggleLineNumbers => {
            state.show_line_numbers = !state.show_line_numbers;
            state.set_status(if state.show_line_numbers {
                "已显示代码行号"
            } else {
                "已隐藏代码行号，复制多行代码时更干净"
            });
            ReduceResult::none()
        }
        Action::FocusExplorer => {
            state.focus = FocusPane::Explorer;
            ReduceResult::none()
        }
        Action::OpenExternal => {
            if let Some(path) = state.selected_entry_path().map(PathBuf::from) {
                return ReduceResult {
                    effects: vec![Effect::OpenExternal(path)],
                    should_quit: false,
                };
            }
            ReduceResult::none()
        }
        Action::Status(message) => {
            state.set_status(message);
            ReduceResult::none()
        }
        Action::Quit => ReduceResult {
            effects: Vec::new(),
            should_quit: true,
        },
    }
}

fn handle_command(state: &mut AppState, command: &str) -> ReduceResult {
    match command {
        "" => ReduceResult::none(),
        "q" | "quit" => reduce(state, Action::Quit),
        "help" => reduce(state, Action::OpenHelp),
        "refresh" => reduce(state, Action::Refresh),
        "toggle hidden" => reduce(state, Action::ToggleHidden),
        "toggle numbers" => reduce(state, Action::ToggleLineNumbers),
        "bookmark list" => reduce(state, Action::OpenBookmarkPicker),
        other if other.starts_with("bookmark add") => {
            let name = other.trim_start_matches("bookmark add").trim();
            let path = state
                .selected_entry()
                .filter(|entry| matches!(entry.kind, EntryKind::Directory))
                .map(|entry| entry.path.clone())
                .unwrap_or_else(|| state.cwd.clone());
            let name = if name.is_empty() {
                path.file_name()
                    .map(|value| value.to_string_lossy().to_string())
                    .unwrap_or_else(|| "根目录".to_string())
            } else {
                name.to_string()
            };
            ReduceResult {
                effects: vec![Effect::SaveBookmark { name, path }],
                should_quit: false,
            }
        }
        other if other.starts_with("bookmark jump ") => {
            let name = other.trim_start_matches("bookmark jump ").trim();
            if name.is_empty() {
                state.set_status("bookmark jump 需要提供名称");
                return ReduceResult::none();
            }
            ReduceResult {
                effects: vec![Effect::JumpBookmark {
                    name: name.to_string(),
                    include_hidden: state.include_hidden,
                }],
                should_quit: false,
            }
        }
        "note edit" => reduce(state, Action::OpenNoteEditor),
        "note clear" => {
            if let Some(path) = state.selected_entry_path().map(PathBuf::from) {
                state.overlay = OverlayState::Confirm {
                    title: "删除备注".to_string(),
                    message: format!("确定删除备注吗？\n{}", path.display()),
                    intent: ConfirmIntent::ClearNote { path },
                    return_to: None,
                };
                ReduceResult::none()
            } else {
                ReduceResult::none()
            }
        }
        "sort name" => reduce(state, Action::SetSort(SortMode::Name)),
        "sort mtime" => reduce(state, Action::SetSort(SortMode::Mtime)),
        "inspect" => reduce(state, Action::OpenReader),
        "open external" => reduce(state, Action::OpenExternal),
        _ => {
            state.set_status(format!("未知命令: {command}"));
            ReduceResult::none()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_overlay_restores_previous_overlay() {
        let mut state = AppState::new(PathBuf::from("/tmp"), None);
        state.overlay = OverlayState::NoteEditor {
            path: PathBuf::from("/tmp/demo.txt"),
            input: "备注".to_string(),
            original: Some("备注".to_string()),
        };

        reduce(&mut state, Action::OpenHelp);
        assert!(matches!(state.overlay, OverlayState::Help { .. }));

        reduce(&mut state, Action::CloseOverlay);
        assert!(matches!(state.overlay, OverlayState::NoteEditor { .. }));
    }

    #[test]
    fn toggle_line_numbers_updates_status() {
        let mut state = AppState::new(PathBuf::from("/tmp"), None);
        reduce(&mut state, Action::ToggleLineNumbers);
        assert!(!state.show_line_numbers);
        assert!(state.status.contains("已隐藏"));
    }

    #[test]
    fn horizontal_preview_scroll_updates_state() {
        let mut state = AppState::new(PathBuf::from("/tmp"), None);
        reduce(&mut state, Action::ScrollPreviewHorizontal(6));
        assert_eq!(state.preview_scroll_x, 6);

        reduce(&mut state, Action::ScrollPreviewHorizontal(-10));
        assert_eq!(state.preview_scroll_x, 0);
    }

    #[test]
    fn submit_search_keeps_filter_and_returns_to_normal_mode() {
        let mut state = AppState::new(PathBuf::from("/tmp"), None);
        state.input_mode = InputMode::Search;
        state.search_input = "src".to_string();
        state.apply_filter();

        reduce(&mut state, Action::SubmitSearch);

        assert!(matches!(state.input_mode, InputMode::Normal));
        assert_eq!(state.search_input, "src");
        assert!(state.status.contains("已应用搜索"));
    }

    #[test]
    fn clear_search_clears_filter_and_returns_to_normal_mode() {
        let mut state = AppState::new(PathBuf::from("/tmp"), None);
        state.input_mode = InputMode::Search;
        state.search_input = "src".to_string();
        state.apply_filter();

        reduce(&mut state, Action::ClearSearch);

        assert!(matches!(state.input_mode, InputMode::Normal));
        assert!(state.search_input.is_empty());
        assert!(state.status.contains("已清除过滤"));
    }
}
