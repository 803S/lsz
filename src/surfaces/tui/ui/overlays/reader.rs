use crate::{
    domain::preview::{PreviewModel, PreviewWrapMode},
    surfaces::tui::{app::state::AppState, ui::preview_document_lines_to_tui},
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::Line,
    widgets::{Clear, Paragraph, Wrap},
};

pub fn render(frame: &mut Frame, state: &AppState) {
    let area = frame.size();
    frame.render_widget(Clear, area);
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);
    let (title, lines, wrap_mode) = match &state.preview {
        PreviewModel::Loading { path } => (
            format!("阅读  {}", path.display()),
            vec![Line::from("预览加载中...")],
            PreviewWrapMode::Wrap,
        ),
        PreviewModel::Failed { path, message } => (
            format!("阅读  {}", path.display()),
            vec![
                Line::from(format!("预览失败: {message}")),
                Line::from(path.display().to_string()),
            ],
            PreviewWrapMode::Wrap,
        ),
        PreviewModel::Document(document) => (
            format!(
                "阅读  {} [{}:{}]",
                document.title,
                document.kind.label(),
                document.provider_name
            ),
            preview_document_lines_to_tui(document, state.show_line_numbers),
            document.wrap_mode,
        ),
    };
    frame.render_widget(
        Paragraph::new(title).style(
            Style::default()
                .fg(state.theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        layout[0],
    );
    let paragraph = Paragraph::new(lines).scroll((state.preview_scroll, state.preview_scroll_x));
    if wrap_mode.should_wrap() {
        frame.render_widget(paragraph.wrap(Wrap { trim: false }), layout[1]);
    } else {
        frame.render_widget(paragraph, layout[1]);
    }
    frame.render_widget(
        Paragraph::new("j/k 滚动  Left/Right 或 H/L 横移  n 行号  ? 帮助  q 关闭"),
        layout[2],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::preview::{PreviewDocument, PreviewKind, PreviewLine},
        surfaces::tui::app::state::{AppState, OverlayState},
    };
    use ratatui::{Terminal, backend::TestBackend};
    use std::path::PathBuf;

    fn reader_state(show_line_numbers: bool) -> AppState {
        let mut state = AppState::new(PathBuf::from("/tmp"), None);
        state.overlay = OverlayState::Reader;
        state.show_line_numbers = show_line_numbers;
        state.preview = PreviewModel::Document(PreviewDocument {
            kind: PreviewKind::Code,
            title: "demo.rs".to_string(),
            subtitle: None,
            provider_name: "文本".to_string(),
            degraded: false,
            metadata: Vec::new(),
            warnings: Vec::new(),
            actions: Vec::new(),
            wrap_mode: PreviewWrapMode::NoWrap,
            lines: vec![PreviewLine::plain("alpha();")],
            first_line_number: 1,
        });
        state
    }

    fn row_text(terminal: &Terminal<TestBackend>, row: u16) -> String {
        let buffer = terminal.backend().buffer();
        (0..buffer.area.width)
            .map(|x| buffer.get(x, row).symbol())
            .collect::<String>()
    }

    #[test]
    fn reader_body_starts_at_left_edge_without_side_borders() {
        let backend = TestBackend::new(40, 5);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let state = reader_state(false);

        terminal.draw(|frame| render(frame, &state)).expect("draw");

        let content_row = row_text(&terminal, 1);
        assert!(content_row.starts_with("alpha();"));
        assert!(!content_row.starts_with('│'));
    }

    #[test]
    fn reader_shows_line_numbers_only_when_enabled() {
        let backend = TestBackend::new(40, 5);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let state = reader_state(true);

        terminal.draw(|frame| render(frame, &state)).expect("draw");

        let content_row = row_text(&terminal, 1);
        assert!(content_row.starts_with("   1 │ alpha();"));
    }
}
