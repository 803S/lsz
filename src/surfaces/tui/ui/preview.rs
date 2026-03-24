use crate::{
    domain::preview::{PreviewModel, PreviewWrapMode},
    surfaces::tui::{
        app::state::{AppState, FocusPane},
        ui::preview_document_lines_to_tui,
    },
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
};

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let title = match &state.preview {
        PreviewModel::Loading { path } => format!(" 预览  {}", path.display()),
        PreviewModel::Failed { path, .. } => format!(" 预览  {}", path.display()),
        PreviewModel::Document(document) => {
            format!(
                " 预览  {} [{}:{}]",
                document.title,
                document.kind.label(),
                document.provider_name
            )
        }
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(if state.focus == FocusPane::Preview {
            Style::default().fg(state.theme.accent)
        } else {
            Style::default()
        });
    let (lines, wrap_mode) = match &state.preview {
        PreviewModel::Loading { .. } => (vec![Line::from("预览加载中...")], PreviewWrapMode::Wrap),
        PreviewModel::Failed { path, message } => (
            vec![
                Line::from(format!("预览失败: {message}")),
                Line::from(path.display().to_string()),
            ],
            PreviewWrapMode::Wrap,
        ),
        PreviewModel::Document(document) => (
            preview_document_lines_to_tui(document, state.show_line_numbers),
            document.wrap_mode,
        ),
    };
    let paragraph = Paragraph::new(lines)
        .block(block)
        .scroll((state.preview_scroll, state.preview_scroll_x));
    if wrap_mode.should_wrap() {
        frame.render_widget(paragraph.wrap(Wrap { trim: false }), area);
    } else {
        frame.render_widget(paragraph, area);
    }
}
