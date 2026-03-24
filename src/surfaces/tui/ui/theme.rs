use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::Block,
};

pub fn render_overlay_backdrop(frame: &mut Frame, area: Rect) {
    frame.render_widget(
        Block::default().style(Style::default().bg(Color::Rgb(8, 8, 8))),
        area,
    );
}
