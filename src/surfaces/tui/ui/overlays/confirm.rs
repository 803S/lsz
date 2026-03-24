use crate::surfaces::tui::{
    app::state::{AppState, OverlayState},
    ui::theme,
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

pub fn render(frame: &mut Frame, state: &AppState) {
    let OverlayState::Confirm { title, message, .. } = &state.overlay else {
        return;
    };
    theme::render_overlay_backdrop(frame, frame.size());
    let area = centered_rect(54, 26, frame.size());
    frame.render_widget(Clear, area);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(area);
    frame.render_widget(
        Paragraph::new(message.as_str())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" {title} ")),
            )
            .wrap(Wrap { trim: false }),
        sections[0],
    );
    frame.render_widget(
        Paragraph::new("Enter / y 确认  Esc / n 取消  ? 帮助"),
        sections[1],
    );
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
