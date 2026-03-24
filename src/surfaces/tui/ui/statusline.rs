use crate::{
    config::keymap::context_label,
    surfaces::tui::{
        app::state::{AppState, InputMode},
        ui::commandline,
    },
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_width::UnicodeWidthStr;

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let left = match state.input_mode {
        InputMode::Search | InputMode::Command => commandline::current_prompt(state),
        InputMode::Normal => {
            let max_items = if area.width >= 108 {
                6
            } else if area.width >= 84 {
                4
            } else {
                3
            };
            let bindings = state.footer_bindings();
            bindings
                .into_iter()
                .take(max_items)
                .map(|binding| format!("{} {}", binding.keys.join("/"), binding.label))
                .collect::<Vec<_>>()
                .join(" | ")
        }
    };
    let right = if state.input_mode == InputMode::Command {
        "命令: help | sort name | sort mtime | bookmark add [name] | note clear".to_string()
    } else if state.input_mode == InputMode::Search {
        "搜索名称 / path / 备注 / 摘要，Enter 应用，Esc 清空".to_string()
    } else if !state.search_input.is_empty() {
        if area.width >= 96 {
            format!(
                "过滤中: {}  Esc 清除  / 修改  {}/{}",
                state.search_input,
                state.selected.map(|idx| idx + 1).unwrap_or(0),
                state.visible_len()
            )
        } else {
            format!("过滤中: {}  Esc 清除", state.search_input)
        }
    } else if state.status.is_empty() {
        if area.width >= 84 {
            format!(
                "{}  排序:{}  行号:{}  {}/{}",
                context_label(state.current_help_context()),
                state.sort_mode_label(),
                if state.show_line_numbers {
                    "开"
                } else {
                    "关"
                },
                state.selected.map(|idx| idx + 1).unwrap_or(0),
                state.visible_len()
            )
        } else {
            format!(
                "{}  {}/{}",
                context_label(state.current_help_context()),
                state.selected.map(|idx| idx + 1).unwrap_or(0),
                state.visible_len()
            )
        }
    } else {
        state.status.clone()
    };
    let total_width = area.width as usize;
    let left_width = display_width(&left);
    let right_width = display_width(&right);
    let gap = 2usize;
    let (left, right) = if left_width + right_width + gap <= total_width {
        (left, right)
    } else {
        let reserved_for_right = right_width.min(total_width / 2);
        let left_budget = total_width.saturating_sub(reserved_for_right + gap);
        (
            truncate_to_width(&left, left_budget),
            truncate_to_width(&right, total_width.saturating_sub(left_budget + gap)),
        )
    };
    let status_style = if state.status.contains("失败") || state.status.contains("错误") {
        Style::default().fg(state.theme.error)
    } else if state.status.starts_with("已") || state.status.starts_with("排序") {
        Style::default().fg(state.theme.success)
    } else if state.status.starts_with("正在")
        || state.status.ends_with("模式")
        || state.input_mode == InputMode::Command
        || state.input_mode == InputMode::Search
        || (state.input_mode == InputMode::Normal && !state.search_input.is_empty())
    {
        Style::default().fg(state.theme.accent)
    } else {
        Style::default().fg(state.theme.muted)
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(left, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::styled(right, status_style),
        ])),
        area,
    );
}

fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

fn truncate_to_width(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    if display_width(text) <= max_width {
        return text.to_string();
    }
    if max_width <= 1 {
        return "…".to_string();
    }
    let mut output = String::new();
    for ch in text.chars() {
        let next = format!("{output}{ch}");
        if display_width(&next) + 1 > max_width {
            break;
        }
        output.push(ch);
    }
    output.push('…');
    output
}
