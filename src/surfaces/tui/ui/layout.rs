use ratatui::layout::{Constraint, Direction, Layout, Rect};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    Wide,
    Stacked,
    Rows,
    Compact,
    Micro,
}

pub struct Areas {
    pub explorer: Rect,
    pub preview: Rect,
    pub inspector: Rect,
    pub status: Rect,
    pub mode: LayoutMode,
}

pub fn split(area: Rect) -> Areas {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);
    if area.width >= 136 && area.height >= 18 {
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(28),
                Constraint::Percentage(48),
                Constraint::Percentage(24),
            ])
            .split(vertical[0]);
        Areas {
            explorer: horizontal[0],
            preview: horizontal[1],
            inspector: horizontal[2],
            status: vertical[1],
            mode: LayoutMode::Wide,
        }
    } else if area.width >= 100 && area.height >= 18 {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(34), Constraint::Percentage(66)])
            .split(vertical[0]);
        let right = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
            .split(columns[1]);
        Areas {
            explorer: columns[0],
            preview: right[0],
            inspector: right[1],
            status: vertical[1],
            mode: LayoutMode::Stacked,
        }
    } else if area.width >= 72 && area.height >= 18 {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(38),
                Constraint::Percentage(34),
                Constraint::Percentage(28),
            ])
            .split(vertical[0]);
        Areas {
            explorer: rows[0],
            preview: rows[1],
            inspector: rows[2],
            status: vertical[1],
            mode: LayoutMode::Rows,
        }
    } else if area.height >= 14 {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
            .split(vertical[0]);
        Areas {
            explorer: rows[0],
            preview: rows[1],
            inspector: rows[1],
            status: vertical[1],
            mode: LayoutMode::Compact,
        }
    } else {
        Areas {
            explorer: vertical[0],
            preview: vertical[0],
            inspector: vertical[0],
            status: vertical[1],
            mode: LayoutMode::Micro,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_layout_keeps_three_columns() {
        let areas = split(Rect::new(0, 0, 160, 40));
        assert_eq!(areas.mode, LayoutMode::Wide);
        assert!(areas.explorer.width > 0);
        assert!(areas.preview.width > 0);
        assert!(areas.inspector.width > 0);
    }

    #[test]
    fn stacked_layout_moves_preview_and_inspector_vertically() {
        let areas = split(Rect::new(0, 0, 110, 40));
        assert_eq!(areas.mode, LayoutMode::Stacked);
        assert_eq!(areas.preview.x, areas.inspector.x);
        assert!(areas.preview.y < areas.inspector.y);
    }

    #[test]
    fn rows_layout_keeps_all_panes_visible() {
        let areas = split(Rect::new(0, 0, 80, 32));
        assert_eq!(areas.mode, LayoutMode::Rows);
        assert!(areas.explorer.y < areas.preview.y);
        assert!(areas.preview.y < areas.inspector.y);
    }
}
