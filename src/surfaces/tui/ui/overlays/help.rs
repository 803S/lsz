use crate::{
    config::keymap::{category_label, context_label},
    surfaces::tui::{
        app::state::{AppState, OverlayState},
        ui::theme,
    },
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};

pub fn render(frame: &mut Frame, state: &AppState) {
    theme::render_overlay_backdrop(frame, frame.size());
    let area = centered_rect(78, 78, frame.size());
    frame.render_widget(Clear, area);

    let (context, filter, scroll, selected_category) = match &state.overlay {
        OverlayState::Help {
            filter,
            scroll,
            context,
            category,
        } => (*context, filter.as_str(), *scroll, *category),
        _ => return,
    };
    let search_all_categories = !filter.trim().is_empty();

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(area);
    let body = if area.width >= 90 {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(18), Constraint::Min(20)])
            .split(sections[1])
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(8), Constraint::Min(10)])
            .split(sections[1])
    };

    let title = Line::from(vec![
        Span::styled("帮助", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(
            context_label(context),
            Style::default().fg(state.theme.accent),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(vec![
            title,
            Line::from(if search_all_categories {
                "方向键滚动；已跨分类搜索；支持拼音和模糊搜索；Esc 关闭"
            } else {
                "方向键滚动，Left/Right 或 Tab 切换分类，Esc 关闭"
            }),
        ])
        .block(Block::default().borders(Borders::ALL).title(" 快捷键帮助 ")),
        sections[0],
    );

    let categories = state.help_categories_for(context);
    let category_items = categories
        .iter()
        .map(|category| ListItem::new(category_label(*category)))
        .collect::<Vec<_>>();
    let mut category_state = ListState::default();
    category_state.select(
        categories
            .iter()
            .position(|category| *category == selected_category),
    );
    frame.render_stateful_widget(
        List::new(category_items)
            .block(Block::default().borders(Borders::ALL).title(" 分类 "))
            .highlight_symbol("▎")
            .highlight_style(Style::default().fg(state.theme.accent)),
        body[0],
        &mut category_state,
    );

    let mut lines = Vec::new();
    for binding in state.help_bindings() {
        let mut spans = Vec::new();
        if search_all_categories {
            spans.push(Span::styled(
                format!("[{}] ", category_label(binding.category)),
                Style::default().fg(state.theme.muted),
            ));
        }
        spans.push(Span::styled(
            format!("{:18}", binding.keys.join(", ")),
            Style::default().fg(state.theme.accent),
        ));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!("{:10}", binding.label),
            if binding.primary {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            },
        ));
        spans.push(Span::raw(" "));
        spans.push(Span::raw(binding.detail));
        lines.push(Line::from(spans));
    }
    if lines.is_empty() {
        lines.push(Line::from("当前过滤条件下没有匹配项"));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(if search_all_categories {
                        " 匹配结果 "
                    } else {
                        " 键位与说明 "
                    }),
            )
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0))
            .alignment(Alignment::Left),
        body[1],
    );

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(format!(
                "过滤关键字: {}",
                if filter.is_empty() {
                    "未输入"
                } else {
                    filter
                }
            )),
            Line::from("支持按键名、动作名、说明、分类；支持拼音与模糊搜索"),
        ])
        .block(Block::default().borders(Borders::ALL).title(" 过滤器 ")),
        sections[2],
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
