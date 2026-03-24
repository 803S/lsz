use crate::surfaces::tui::{
    app::state::{AppState, OverlayState},
    ui::theme,
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

pub fn render(frame: &mut Frame, state: &AppState) {
    let OverlayState::BookmarkPicker { items, selected } = &state.overlay else {
        return;
    };
    theme::render_overlay_backdrop(frame, frame.size());
    let area = centered_rect(60, 50, frame.size());
    frame.render_widget(Clear, area);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);
    let list_items: Vec<ListItem> = items
        .iter()
        .map(|bookmark| {
            let detail = bookmark
                .note
                .as_deref()
                .map(|note| format!("  -- {note}"))
                .unwrap_or_default();
            ListItem::new(format!(
                "{} -> {}{}",
                bookmark.name,
                bookmark.path.display(),
                detail
            ))
        })
        .collect();
    let mut list_state = ListState::default();
    list_state.select(Some(*selected));
    frame.render_stateful_widget(
        List::new(list_items)
            .block(Block::default().borders(Borders::ALL).title(" 书签 "))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD))
            .highlight_symbol("▎"),
        sections[0],
        &mut list_state,
    );
    frame.render_widget(
        Paragraph::new("Enter 跳转  d 删除  Esc 关闭  ? 帮助"),
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
