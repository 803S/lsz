use crate::{
    domain::entry::EntryKind,
    surfaces::tui::app::state::{AppState, FocusPane},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
};

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let items: Vec<ListItem> = state
        .filtered
        .iter()
        .filter_map(|idx| state.entries.get(*idx))
        .map(|entry| {
            let color = match entry.kind {
                EntryKind::Directory => Color::Blue,
                EntryKind::Markdown => Color::Cyan,
                EntryKind::Code => Color::Green,
                EntryKind::Pdf => Color::Red,
                EntryKind::Archive => Color::Yellow,
                EntryKind::Image => Color::Magenta,
                EntryKind::Text => Color::White,
                _ => Color::Gray,
            };
            let mut spans = vec![Span::styled(
                entry.display_name(),
                Style::default().fg(color),
            )];
            if entry.note.is_some() || entry.auto_summary.is_some() {
                spans.push(Span::styled(" •", Style::default().fg(Color::Yellow)));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(state.selected);
    let title = format!(" 浏览  {}", state.cwd.display());
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(if state.focus == FocusPane::Explorer {
            Style::default().fg(state.theme.accent)
        } else {
            Style::default()
        });
    frame.render_stateful_widget(
        List::new(items)
            .block(block)
            .highlight_style(
                Style::default()
                    .bg(state.theme.selection_bg)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▎"),
        area,
        &mut list_state,
    );
}
