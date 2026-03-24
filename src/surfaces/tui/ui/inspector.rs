use crate::{
    domain::preview::PreviewModel,
    infra::fs,
    surfaces::tui::app::state::{AppState, FocusPane},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use std::collections::HashSet;

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" 信息 ")
        .border_style(if state.focus == FocusPane::Inspector {
            Style::default().fg(state.theme.accent)
        } else {
            Style::default()
        });
    let mut lines = Vec::new();
    if let Some(entry) = state.selected_entry() {
        lines.push(Line::from(vec![Span::styled(
            entry.display_name(),
            Style::default().fg(Color::Cyan),
        )]));
        lines.push(Line::from(""));
        lines.push(Line::from(format!("路径: {}", entry.path.display())));
        lines.push(Line::from(format!("大小: {}", fs::size_label(entry.size))));
        lines.push(Line::from(format!(
            "修改时间: {}",
            fs::time_label(entry.mtime)
        )));
        lines.push(Line::from(format!("隐藏文件: {}", entry.is_hidden)));
        if let Some(note) = &entry.note {
            lines.push(Line::from(""));
            lines.push(Line::from("备注:"));
            lines.push(Line::from(note.clone()));
        }
        if let Some(summary) = &entry.auto_summary {
            lines.push(Line::from(""));
            lines.push(Line::from("摘要:"));
            lines.push(Line::from(summary.clone()));
        }
    }

    if let PreviewModel::Document(document) = &state.preview {
        lines.push(Line::from(""));
        lines.push(Line::from(format!(
            "预览器: {}{}",
            document.provider_name,
            if document.degraded {
                "（已降级）"
            } else {
                ""
            }
        )));
        lines.push(Line::from(format!("类型: {}", document.kind.label())));
        if !document.metadata.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from("元信息:"));
            let mut shown_keys = HashSet::from([normalize_meta_key("大小")]);
            for (key, value) in &document.metadata {
                let normalized = normalize_meta_key(key);
                if !shown_keys.insert(normalized) {
                    continue;
                }
                lines.push(Line::from(format!("{key}: {value}")));
            }
        }
        if !document.warnings.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from("警告:"));
            for warning in &document.warnings {
                lines.push(Line::from(Span::styled(
                    warning.clone(),
                    Style::default().fg(state.theme.warning),
                )));
            }
        }
        if !document.actions.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from("操作:"));
            for action in &document.actions {
                lines.push(Line::from(action.clone()));
            }
        }
    }
    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((state.preview_scroll, 0)),
        area,
    );
}

fn normalize_meta_key(key: &str) -> String {
    key.to_ascii_lowercase()
}
