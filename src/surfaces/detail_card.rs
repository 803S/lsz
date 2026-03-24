use anyhow::{Result, anyhow};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, MouseEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};
use std::{
    collections::HashSet,
    fs,
    io::IsTerminal,
    path::{Path, PathBuf},
    time::Duration,
};
use unicode_width::UnicodeWidthStr;

use crate::{
    domain::preview::{PreviewModel, PreviewWrapMode},
    infra::{
        db::Database,
        fs as app_fs,
        providers::{PreviewService, PreviewSurface},
    },
    surfaces::tui::ui::preview_line_to_tui,
};

type AppTerminal = Terminal<CrosstermBackend<std::io::Stdout>>;

struct TerminalGuard {
    terminal: AppTerminal,
}

impl TerminalGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

struct DetailCardData {
    path_str: String,
    header_lines: Vec<Line<'static>>,
    info_pairs: Vec<(String, String)>,
    warning_lines: Vec<Line<'static>>,
    preview_title: String,
    preview_lines: Vec<Line<'static>>,
    wrap_preview: bool,
    scroll_y: u16,
}

pub fn run(path: &str) -> Result<()> {
    if !std::io::stdout().is_terminal() {
        return Err(anyhow!(
            "`lsz -i` 需要在 TTY 终端中运行；如果只是导出文本，请改用 `lsz --plain [path]`"
        ));
    }

    let path = PathBuf::from(path);
    if !path.exists() {
        return Err(anyhow!("路径不存在: {}", path.display()));
    }

    let db = Database::open_best_effort()?;
    let path = fs::canonicalize(path)?;
    let meta = fs::metadata(&path)?;
    let note = db.resolve_note(&path)?;
    let summary = if meta.is_dir() {
        app_fs::auto_summary(&path)
    } else {
        None
    };

    let mut info_pairs = vec![
        (
            "类型".to_string(),
            if meta.is_dir() {
                "目录".to_string()
            } else {
                "文件".to_string()
            },
        ),
        ("大小".to_string(), app_fs::size_label(Some(meta.len()))),
        ("修改".to_string(), app_fs::time_label(meta.modified().ok())),
    ];

    if meta.is_dir() {
        let mut dir_count = 0usize;
        let mut file_count = 0usize;
        if let Ok(entries) = fs::read_dir(&path) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    dir_count += 1;
                } else {
                    file_count += 1;
                }
            }
        }
        info_pairs.push((
            "子项".to_string(),
            format!("目录 {dir_count} / 文件 {file_count}"),
        ));
    }

    let mut header_lines = vec![Line::from(Span::styled(
        path.display().to_string(),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ))];
    if let Some(summary) = summary {
        header_lines.push(labeled_line("摘要", &summary, Color::White));
    }
    if let Some(note) = note {
        header_lines.push(labeled_line("备注", &note, Color::Yellow));
    }
    if header_lines.len() == 1 {
        header_lines.push(Line::from(Span::styled(
            "README / AGENTS / GUIDE 等说明文档优先预览",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let (preview_path, preview_source) = inspect_preview_target(&path, &meta);
    let preview =
        PreviewService::default().preview_path(&preview_path, 80, PreviewSurface::Inspect);
    let mut preview_title = "预览".to_string();
    let mut preview_lines = vec![Line::from("预览加载中...")];
    let mut warning_lines = Vec::new();
    let mut wrap_preview = true;

    match preview {
        PreviewModel::Document(document) => {
            if let Some(source) = preview_source {
                info_pairs.push(("默认文档".to_string(), source.clone()));
                preview_title = format!("{source} 预览");
            } else {
                preview_title = format!("{} 预览", document.kind.label());
            }

            info_pairs.push((
                "预览".to_string(),
                format!(
                    "{}{}",
                    document.provider_name,
                    if document.degraded {
                        "（已降级）"
                    } else {
                        ""
                    }
                ),
            ));

            let mut shown_keys = HashSet::from([normalize_meta_key("大小")]);
            for (key, value) in document.metadata {
                let normalized = normalize_meta_key(&key);
                if !shown_keys.insert(normalized) {
                    continue;
                }
                info_pairs.push((key, value));
            }

            warning_lines = document
                .warnings
                .into_iter()
                .map(|warning| {
                    Line::from(vec![
                        Span::styled("提示 ", Style::default().fg(Color::Yellow)),
                        Span::raw(warning),
                    ])
                })
                .collect();
            preview_lines = document
                .lines
                .into_iter()
                .map(|line| preview_line_to_tui(&line))
                .collect();
            if preview_lines.is_empty() {
                preview_lines.push(Line::from("（空预览）"));
            }
            wrap_preview = matches!(document.wrap_mode, PreviewWrapMode::Wrap);
        }
        PreviewModel::Failed { message, .. } => {
            preview_lines = vec![Line::from(format!("预览失败: {message}"))];
        }
        PreviewModel::Loading { .. } => {}
    }

    let mut data = DetailCardData {
        path_str: path.display().to_string(),
        header_lines,
        info_pairs,
        warning_lines,
        preview_title,
        preview_lines,
        wrap_preview,
        scroll_y: 0,
    };

    let mut terminal = TerminalGuard::enter()?;
    loop {
        terminal.terminal.draw(|frame| draw_card(frame, &data))?;
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('j') | KeyCode::Down => {
                        data.scroll_y = data.scroll_y.saturating_add(1)
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        data.scroll_y = data.scroll_y.saturating_sub(1)
                    }
                    KeyCode::PageDown => data.scroll_y = data.scroll_y.saturating_add(10),
                    KeyCode::PageUp => data.scroll_y = data.scroll_y.saturating_sub(10),
                    _ => {}
                },
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollDown => data.scroll_y = data.scroll_y.saturating_add(3),
                    MouseEventKind::ScrollUp => data.scroll_y = data.scroll_y.saturating_sub(3),
                    _ => {}
                },
                _ => {}
            }
        }
    }
    Ok(())
}

fn draw_card(frame: &mut ratatui::Frame, data: &DetailCardData) {
    let area = centered_card_rect(frame.size());
    frame.render_widget(Clear, area);

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" 项目卡片 ")
        .border_style(Style::default().fg(Color::Cyan));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let info_lines = info_pairs_to_lines(&data.info_pairs, inner.width);
    let header_height = data.header_lines.len() as u16 + 1;
    let info_height = info_lines.len() as u16 + 1;
    let warning_height = if data.warning_lines.is_empty() {
        0
    } else {
        data.warning_lines.len() as u16 + 1
    };

    let mut constraints = vec![
        Constraint::Length(header_height.max(3)),
        Constraint::Length(info_height.max(3)),
    ];
    if warning_height > 0 {
        constraints.push(Constraint::Length(warning_height));
    }
    constraints.push(Constraint::Min(0));
    constraints.push(Constraint::Length(1));

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    let mut index = 0usize;
    frame.render_widget(
        Paragraph::new(data.header_lines.clone())
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .title(" 路径与摘要 "),
            )
            .wrap(Wrap { trim: false }),
        sections[index],
    );
    index += 1;

    frame.render_widget(
        Paragraph::new(info_lines)
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .title(" 关键信息 "),
            )
            .wrap(Wrap { trim: false }),
        sections[index],
    );
    index += 1;

    if warning_height > 0 {
        frame.render_widget(
            Paragraph::new(data.warning_lines.clone())
                .block(Block::default().borders(Borders::BOTTOM).title(" 提示 "))
                .wrap(Wrap { trim: false }),
            sections[index],
        );
        index += 1;
    }

    let preview = Paragraph::new(data.preview_lines.clone())
        .block(
            Block::default()
                .borders(Borders::TOP)
                .title(format!(" {} ", data.preview_title)),
        )
        .scroll((data.scroll_y, 0));
    if data.wrap_preview {
        frame.render_widget(preview.wrap(Wrap { trim: false }), sections[index]);
    } else {
        frame.render_widget(preview, sections[index]);
    }

    frame.render_widget(
        Paragraph::new(format!("j/k 滚动  q 退出  {}", data.path_str))
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray)),
        sections[index + 1],
    );
}

fn inspect_preview_target(path: &Path, meta: &fs::Metadata) -> (PathBuf, Option<String>) {
    if meta.is_dir() {
        if let Some(doc) = app_fs::find_project_doc(path) {
            let label = doc
                .file_name()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_else(|| "项目说明".to_string());
            return (doc, Some(label));
        }
    }
    (path.to_path_buf(), None)
}

fn labeled_line(label: &str, value: &str, value_color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label} "), Style::default().fg(Color::DarkGray)),
        Span::styled(value.to_string(), Style::default().fg(value_color)),
    ])
}

fn info_pairs_to_lines(pairs: &[(String, String)], width: u16) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let columns = if width >= 132 {
        3
    } else if width >= 84 {
        2
    } else {
        1
    };
    let label_width = 8usize;
    let cell_width = width.saturating_sub(2) as usize / columns.max(1);

    let mut index = 0usize;
    while index < pairs.len() {
        let mut spans = Vec::new();
        for column in 0..columns {
            let Some((label, value)) = pairs.get(index + column) else {
                break;
            };
            let content = pad_pair(label, value, label_width, cell_width.saturating_sub(2));
            spans.push(Span::raw(content));
            if column + 1 < columns {
                spans.push(Span::raw("  "));
            }
        }
        lines.push(Line::from(spans));
        index += columns;
    }

    lines
}

fn pad_pair(label: &str, value: &str, label_width: usize, total_width: usize) -> String {
    let left = format!("{label:<label_width$}");
    let combined = format!("{left}{value}");
    let width = UnicodeWidthStr::width(combined.as_str());
    if width >= total_width {
        combined
    } else {
        format!("{combined}{}", " ".repeat(total_width - width))
    }
}

fn centered_card_rect(area: Rect) -> Rect {
    let width = if area.width > 168 {
        area.width.saturating_sub(10)
    } else if area.width > 132 {
        area.width.saturating_sub(8)
    } else if area.width > 96 {
        area.width.saturating_sub(6)
    } else {
        area.width.saturating_sub(2).max(16)
    };
    let height = if area.height > 46 {
        area.height.saturating_sub(6)
    } else if area.height > 28 {
        area.height.saturating_sub(4)
    } else {
        area.height.saturating_sub(2).max(10)
    };
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

fn normalize_meta_key(key: &str) -> String {
    key.to_ascii_lowercase()
}
