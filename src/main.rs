// lsz.rs v0.9.6
// 依赖: ratatui 0.26, crossterm 0.27, termimad 0.26, syntect 5.0, ansi-to-tui 4.0, unicode-width 0.1

use ansi_to_tui::IntoText;
use anyhow::Result;
use chrono::{DateTime, Local};
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use rusqlite::{params, Connection, OptionalExtension};
use std::{
    env,
    fs::{self, File},
    io::{BufRead, BufReader, Write},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    time::Duration,
};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style as SynStyle, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};
use termimad::{Area, MadSkin, MadView};
use unicode_width::UnicodeWidthStr;

const DB_FILE: &str = ".lsz.db";
const THEME_COLOR: Color = Color::Cyan;
const SELECTED_BG: Color = Color::DarkGray;

// ==========================================
// Part 0: 辅助函数
// ==========================================

fn print_row(left: &str, right: &str, color: &str, reset: &str) {
    const TARGET_WIDTH: usize = 30;
    let width = left.width_cjk();
    let padding = if width < TARGET_WIDTH {
        TARGET_WIDTH - width
    } else {
        1
    };
    let spaces = " ".repeat(padding);
    println!("  {}{}{}{} {}", color, left, reset, spaces, right);
}

// ==========================================
// Part 1: 核心工具
// ==========================================

fn get_icon(name: &str, is_dir: bool) -> char {
    if is_dir {
        return '📁';
    }
    let ext = Path::new(name)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "rs" => '🦀',
        "py" => '🐍',
        "js" | "ts" | "jsx" | "tsx" => '📜',
        "html" | "vue" => '🌐',
        "css" | "scss" => '🎨',
        "json" | "toml" | "yaml" | "yml" => '⚙',
        "md" | "txt" => '📝',
        "go" => '🐹',
        "c" | "h" => 'C',
        "cpp" => 'ﭱ',
        "java" | "jar" => '☕',
        "sh" | "bash" => '🐚',
        "dockerfile" => '🐳',
        "zip" | "tar" | "gz" | "7z" => '📦',
        "png" | "jpg" | "jpeg" | "svg" | "gif" => '🖼',
        "mp4" | "mov" => '🎬',
        "mp3" | "wav" => '🎵',
        "lock" => '🔒',
        "git" | "gitignore" => '',
        _ => '📄',
    }
}

fn open_db() -> Result<Connection, rusqlite::Error> {
    let home = env::var("HOME").expect("无法获取 HOME");
    let conn = Connection::open(PathBuf::from(home).join(DB_FILE))?;
    conn.execute("CREATE TABLE IF NOT EXISTS notes (dev INTEGER NOT NULL, inode INTEGER NOT NULL, path TEXT NOT NULL, note TEXT NOT NULL, PRIMARY KEY (dev, inode))", [])?;
    Ok(conn)
}

fn resolve_note(conn: &Connection, path: &Path, meta: &fs::Metadata) -> Option<String> {
    let (dev, inode, abs) = (meta.dev(), meta.ino(), path.to_string_lossy().to_string());
    let res: Option<(String, String)> = conn
        .query_row(
            "SELECT note, path FROM notes WHERE dev=?1 AND inode=?2",
            params![dev, inode],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .unwrap_or(None);
    if let Some((note, p)) = res {
        if p != abs {
            conn.execute(
                "UPDATE notes SET path=?1 WHERE dev=?2 AND inode=?3",
                params![abs, dev, inode],
            )
            .ok();
        }
        return Some(note);
    }

    // Fix: using `inspect` instead of `map` + remove needless `Ok`
    conn.query_row("SELECT note FROM notes WHERE path=?1", params![abs], |r| {
        r.get(0)
    })
    .optional()
    .unwrap_or(None)
    .inspect(|_| {
        conn.execute(
            "UPDATE notes SET dev=?1, inode=?2 WHERE path=?3",
            params![dev, inode, abs],
        )
        .ok();
    })
}

fn auto_summary(dir: &Path) -> Option<String> {
    for name in &["README.md", "readme.md", "README.txt"] {
        if let Ok(f) = File::open(dir.join(name)) {
            let mut s = String::new();
            // Fix: manual char comparison
            if BufReader::new(f).read_line(&mut s).unwrap_or(0) > 0 {
                let t = s.trim().trim_start_matches(['#', ' ']).to_string();
                if !t.is_empty() {
                    return Some(t);
                }
            }
        }
    }
    if let Ok(f) = File::open(dir.join("Cargo.toml")) {
        // Fix: lines().flatten() -> lines().map_while(Result::ok)
        for l in BufReader::new(f).lines().map_while(Result::ok).take(20) {
            if l.starts_with("description") {
                if let Some(v) = l.split('=').nth(1) {
                    return Some(v.trim().trim_matches(['"', '\'', ' ']).to_string());
                }
            }
        }
    }
    None
}

fn store_note(p: &str, n: &str) -> Result<()> {
    let path = fs::canonicalize(p)?;
    let meta = fs::metadata(&path)?;
    open_db()?.execute(
        "INSERT OR REPLACE INTO notes (dev, inode, path, note) VALUES (?1, ?2, ?3, ?4)",
        params![meta.dev(), meta.ino(), path.to_string_lossy(), n],
    )?;
    println!("✓ 已保存");
    Ok(())
}

fn delete_note(p: &str) -> Result<()> {
    let path = Path::new(p);
    if !path.exists() {
        return Err(anyhow::anyhow!("路径不存在"));
    }
    let abs_path = fs::canonicalize(path)?;
    let meta = fs::metadata(&abs_path)?;
    let conn = open_db()?;
    let count = conn.execute(
        "DELETE FROM notes WHERE dev=?1 AND inode=?2",
        params![meta.dev(), meta.ino()],
    )?;
    if count > 0 {
        println!("✓ 已删除注释: {}", p);
    } else {
        println!("! 未找到该文件的注释: {}", p);
    }
    Ok(())
}

fn gc_notes() -> Result<()> {
    let conn = open_db()?;
    let mut stmt = conn.prepare("SELECT dev, inode, path FROM notes")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, u64>(0)?,
            r.get::<_, u64>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    let mut d = 0;
    for r in rows {
        let (dev, inode, p) = r?;
        if !Path::new(&p).exists() {
            conn.execute(
                "DELETE FROM notes WHERE dev=?1 AND inode=?2",
                params![dev, inode],
            )?;
            d += 1;
        }
    }
    println!("已清理 {} 条", d);
    Ok(())
}

fn run_simple_list(target: &str) -> Result<()> {
    let path = Path::new(target);
    if path.is_file() {
        println!("{} (文件)", target);
        return Ok(());
    }
    let conn = open_db()?;
    let mut entries = Vec::new();
    let mut max_len = 0;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let p = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        if let Ok(meta) = fs::metadata(&p) {
            let abs = fs::canonicalize(&p).unwrap_or_else(|_| p.clone());
            let mut note = resolve_note(&conn, &abs, &meta);
            let mut is_auto = false;
            if note.is_none() && meta.is_dir() {
                if let Some(s) = auto_summary(&abs) {
                    note = Some(if s.chars().count() > 30 {
                        format!("{}...", s.chars().take(28).collect::<String>())
                    } else {
                        s
                    });
                    is_auto = true;
                }
            }
            let icon = get_icon(&name, meta.is_dir());
            let rl = name.chars().count() + 2;
            if note.is_some() && rl > max_len {
                max_len = rl;
            }
            let color = if meta.is_dir() {
                "\x1b[1;34m"
            } else {
                "\x1b[37m"
            };
            entries.push((
                format!("{} {}{}{}", icon, color, name, "\x1b[0m"),
                rl,
                note,
                is_auto,
            ));
        }
    }
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    let pad = max_len + 2;
    for (disp, rl, note, is_auto) in entries {
        if let Some(n) = note {
            writeln!(
                lock,
                "{}{}{}{}\x1b[0m",
                disp,
                " ".repeat(if pad > rl { pad - rl } else { 2 }),
                if is_auto { "\x1b[90m" } else { "\x1b[33m" },
                n
            )?;
        } else {
            writeln!(lock, "{}", disp)?;
        }
    }
    Ok(())
}

// ==========================================
// Part 2: 高级渲染逻辑
// ==========================================

fn render_markdown_to_ansi(content: &str, width: usize) -> String {
    let skin = MadSkin::default();
    skin.text(content, Some(width)).to_string()
}

fn render_code_styled(path: &Path, content: &str, show_line_numbers: bool) -> String {
    let ps = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();
    let syntax = if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        ps.find_syntax_by_extension(ext)
            .unwrap_or_else(|| ps.find_syntax_plain_text())
    } else {
        ps.find_syntax_plain_text()
    };

    let theme = &ts.themes["base16-ocean.dark"];
    let mut h = HighlightLines::new(syntax, theme);

    let mut output = String::new();
    for (i, line) in LinesWithEndings::from(content).enumerate() {
        if show_line_numbers {
            let prefix = format!("\x1b[90m{:>4} │ \x1b[0m", i + 1);
            output.push_str(&prefix);
        }
        let ranges: Vec<(SynStyle, &str)> = h.highlight_line(line, &ps).unwrap_or_default();
        output.push_str(&as_24_bit_terminal_escaped(&ranges[..], false));
        output.push_str("\x1b[0m");
    }
    output
}

// ==========================================
// Part 3: 详情卡片 (-i)
// ==========================================

struct DetailCardData {
    path_str: String,
    note: Option<String>,
    readme_ansi: Option<String>,
    scroll_y: u16,
}

fn draw_card(f: &mut Frame, data: &DetailCardData) {
    let size = f.size();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Project Card ")
        .style(Style::default().fg(Color::Cyan));
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(block.inner(size));
    f.render_widget(block, size);

    let mut header = vec![Line::from(vec![
        Span::raw("📁 "),
        Span::styled(
            &data.path_str,
            Style::default().add_modifier(Modifier::BOLD),
        ),
    ])];
    if let Some(n) = &data.note {
        header.extend(vec![
            Line::from(""),
            Line::from(vec![
                Span::raw("🏷️  人工备注："),
                Span::styled(n, Style::default().fg(Color::Yellow)),
            ]),
        ]);
    } else {
        header.extend(vec![
            Line::from(""),
            Line::from(Span::styled(
                "   (无备注)",
                Style::default().fg(Color::DarkGray),
            )),
        ]);
    }
    f.render_widget(Paragraph::new(header), chunks[0]);
    f.render_widget(
        Block::default()
            .borders(Borders::TOP)
            .border_type(BorderType::Plain),
        chunks[1],
    );

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(chunks[1]);
    if let Some(ansi) = &data.readme_ansi {
        f.render_widget(
            Paragraph::new("📖 README.md 预览 (↓/滚轮阅读):")
                .style(Style::default().fg(Color::Cyan)),
            inner[0],
        );
        let text = ansi
            .into_text()
            .unwrap_or_else(|_| ratatui::text::Text::raw("解析错误"));
        f.render_widget(
            Paragraph::new(text)
                .wrap(Wrap { trim: false })
                .scroll((data.scroll_y, 0)),
            inner[1],
        );
    } else {
        f.render_widget(
            Paragraph::new("📖 (无 README 文档)").style(Style::default().fg(Color::DarkGray)),
            inner[0],
        );
    }
    f.render_widget(
        Paragraph::new("[ q:退出 | ↓/滚轮:滚动 ]")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray)),
        chunks[2],
    );
}

fn run_detail_card(target: String) -> Result<()> {
    let path = PathBuf::from(&target);
    if !path.exists() {
        return Err(anyhow::anyhow!("路径不存在"));
    }
    let abs = fs::canonicalize(&path)?;
    let meta = fs::metadata(&abs)?;
    let conn = open_db()?;
    let note = resolve_note(&conn, &abs, &meta);
    let mut readme_ansi = None;
    if meta.is_dir() {
        for name in &["README.md", "readme.md", "README.txt"] {
            if let Ok(c) = fs::read_to_string(abs.join(name)) {
                readme_ansi = Some(render_markdown_to_ansi(&c, 100));
                break;
            }
        }
    }
    let mut data = DetailCardData {
        path_str: target,
        note,
        readme_ansi,
        scroll_y: 0,
    };
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    loop {
        terminal.draw(|f| draw_card(f, &data))?;
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(k) if k.kind == KeyEventKind::Press => match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Down | KeyCode::Char('j') => {
                        data.scroll_y = data.scroll_y.saturating_add(1)
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        data.scroll_y = data.scroll_y.saturating_sub(1)
                    }
                    _ => {}
                },
                Event::Mouse(m) => match m.kind {
                    MouseEventKind::ScrollDown => data.scroll_y = data.scroll_y.saturating_add(3),
                    MouseEventKind::ScrollUp => data.scroll_y = data.scroll_y.saturating_sub(3),
                    _ => {}
                },
                _ => {}
            }
        }
    }
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    Ok(())
}

// ==========================================
// Part 4: 交互列表与全屏阅读器
// ==========================================

struct FileEntry {
    name: String,
    path: PathBuf,
    icon: char,
    note: Option<String>,
    is_dir: bool,
    is_auto: bool,
    size: u64,
    modified: String,
}
struct App {
    items: Vec<FileEntry>,
    state: ListState,
    current_dir: PathBuf,
}

impl App {
    fn new(dir: PathBuf) -> Self {
        Self {
            items: Vec::new(),
            state: ListState::default(),
            current_dir: dir,
        }
    }
    fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.items.len().saturating_sub(1) {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }
    fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len().saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }
    fn get_readable_path(&self) -> Option<PathBuf> {
        if let Some(idx) = self.state.selected() {
            let item = &self.items[idx];
            if item.is_dir {
                for name in &["README.md", "readme.md", "README.txt"] {
                    let p = item.path.join(name);
                    if p.exists() {
                        return Some(p);
                    }
                }
            } else {
                return Some(item.path.clone());
            }
        }
        None
    }
    fn load_files(&mut self) -> Result<()> {
        let conn = open_db()?;
        let mut entries = Vec::new();
        if let Ok(read_dir) = fs::read_dir(&self.current_dir) {
            for entry in read_dir.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') {
                    continue;
                }
                if let Ok(meta) = fs::metadata(entry.path()) {
                    let abs = fs::canonicalize(entry.path()).unwrap_or_else(|_| entry.path());
                    let mut note = resolve_note(&conn, &abs, &meta);
                    let mut is_auto = false;
                    if note.is_none() && meta.is_dir() {
                        if let Some(s) = auto_summary(&abs) {
                            note = Some(s);
                            is_auto = true;
                        }
                    }
                    entries.push(FileEntry {
                        name,
                        path: entry.path(),
                        icon: get_icon(&entry.file_name().to_string_lossy(), meta.is_dir()),
                        note,
                        is_dir: meta.is_dir(),
                        is_auto,
                        size: meta.len(),
                        modified: DateTime::<Local>::from(
                            meta.modified().unwrap_or(std::time::SystemTime::now()),
                        )
                        .format("%Y-%m-%d %H:%M")
                        .to_string(),
                    });
                }
            }
        }
        entries.sort_by(|a, b| {
            if a.is_dir != b.is_dir {
                b.is_dir.cmp(&a.is_dir)
            } else {
                a.name.cmp(&b.name)
            }
        });
        self.items = entries;
        if !self.items.is_empty() {
            self.state.select(Some(0));
        }
        Ok(())
    }
}

// Markdown 阅读器
fn run_markdown_viewer(content: String) -> Result<()> {
    disable_raw_mode()?;
    {
        enable_raw_mode()?;
        let mut w = std::io::stdout();
        execute!(
            w,
            crossterm::terminal::Clear(crossterm::terminal::ClearType::All)
        )?;
        let skin = MadSkin::default();
        let mut view = MadView::from(content, Area::full_screen(), skin);
        loop {
            view.write_on(&mut w)?;
            w.flush()?;
            match event::read() {
                Ok(Event::Key(k)) if k.kind == KeyEventKind::Press => match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Down | KeyCode::Char('j') => view.try_scroll_lines(1),
                    KeyCode::Up | KeyCode::Char('k') => view.try_scroll_lines(-1),
                    KeyCode::PageDown => view.try_scroll_pages(1),
                    KeyCode::PageUp => view.try_scroll_pages(-1),
                    _ => {}
                },
                Ok(Event::Mouse(m)) => match m.kind {
                    MouseEventKind::ScrollDown => view.try_scroll_lines(3),
                    MouseEventKind::ScrollUp => view.try_scroll_lines(-3),
                    _ => {}
                },
                _ => {}
            }
        }
    }
    enable_raw_mode()?;
    execute!(std::io::stdout(), EnableMouseCapture)?;
    Ok(())
}

// 代码阅读器
fn run_code_viewer(path: &Path, content: String) -> Result<()> {
    let mut show_lines = true;
    let mut ansi_str = render_code_styled(path, &content, show_lines);
    let mut text = ansi_str
        .clone()
        .into_text()
        .unwrap_or_else(|_| ratatui::text::Text::raw("解析错误"));
    let mut scroll_y = 0;

    let stdout = std::io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    loop {
        terminal.draw(|f| {
            let size = f.size();
            let title = format!(" Reading: {} | Toggle Lines: <n> ", path.display());
            let block = Block::default().borders(Borders::ALL).title(title);
            f.render_widget(
                Paragraph::new(text.clone())
                    .block(block)
                    .scroll((scroll_y, 0)),
                size,
            );
        })?;

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(k) if k.kind == KeyEventKind::Press => match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('n') => {
                        show_lines = !show_lines;
                        ansi_str = render_code_styled(path, &content, show_lines);
                        text = ansi_str
                            .into_text()
                            .unwrap_or_else(|_| ratatui::text::Text::raw("解析错误"));
                    }
                    KeyCode::Down | KeyCode::Char('j') => scroll_y = scroll_y.saturating_add(1),
                    KeyCode::Up | KeyCode::Char('k') => scroll_y = scroll_y.saturating_sub(1),
                    KeyCode::PageDown => scroll_y = scroll_y.saturating_add(20),
                    KeyCode::PageUp => scroll_y = scroll_y.saturating_sub(20),
                    _ => {}
                },
                Event::Mouse(m) => match m.kind {
                    MouseEventKind::ScrollDown => scroll_y = scroll_y.saturating_add(3),
                    MouseEventKind::ScrollUp => scroll_y = scroll_y.saturating_sub(3),
                    _ => {}
                },
                _ => {}
            }
        }
    }
    Ok(())
}

fn open_full_screen_reader(path: &Path) -> Result<()> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };
    // Fix: is_some_and
    let is_md = path.extension().is_some_and(|e| {
        let s = e.to_string_lossy().to_lowercase();
        s == "md" || s == "markdown"
    });

    if is_md {
        run_markdown_viewer(content)
    } else {
        run_code_viewer(path, content)
    }
}

fn run_interactive_list(target_path: PathBuf) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new(target_path);
    if let Err(e) = app.load_files() {
        disable_raw_mode()?;
        execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
        return Err(e);
    }
    loop {
        terminal.draw(|f| ui_list(f, &mut app))?;
        if event::poll(Duration::from_millis(200))? {
            match event::read()? {
                Event::Key(k) if k.kind == KeyEventKind::Press => match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Down | KeyCode::Char('j') => app.next(),
                    KeyCode::Up | KeyCode::Char('k') => app.previous(),
                    KeyCode::Enter => {
                        if let Some(p) = app.get_readable_path() {
                            let _ = open_full_screen_reader(&p);
                            terminal.clear()?;
                            terminal.draw(|f| ui_list(f, &mut app))?;
                        }
                    }
                    _ => {}
                },
                Event::Mouse(m) => match m.kind {
                    MouseEventKind::ScrollDown => app.next(),
                    MouseEventKind::ScrollUp => app.previous(),
                    _ => {}
                },
                _ => {}
            }
        }
    }
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn ui_list(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(f.size());
    f.render_widget(
        Paragraph::new(format!(" 📁 {}", app.current_dir.display())).style(
            Style::default()
                .fg(THEME_COLOR)
                .add_modifier(Modifier::BOLD),
        ),
        chunks[0],
    );
    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);
    let items: Vec<ListItem> = app
        .items
        .iter()
        .map(|i| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{} ", i.icon),
                    Style::default().fg(if i.is_dir { Color::Blue } else { Color::White }),
                ),
                Span::raw(&i.name),
            ]))
        })
        .collect();
    f.render_stateful_widget(
        List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(" Files "),
            )
            .highlight_style(
                Style::default()
                    .bg(SELECTED_BG)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▎"),
        main[0],
        &mut app.state,
    );

    let mut preview: Vec<Line> = Vec::new();
    if let Some(idx) = app.state.selected() {
        if let Some(i) = app.items.get(idx) {
            preview.push(Line::from(vec![Span::styled(
                format!("{} {}", i.icon, i.name),
                Style::default()
                    .fg(THEME_COLOR)
                    .add_modifier(Modifier::BOLD),
            )]));
            preview.push(Line::from(""));
            let size_str = if i.size < 1024 {
                format!("{} B", i.size)
            } else if i.size < 1024 * 1024 {
                format!("{:.1} KB", i.size as f64 / 1024.0)
            } else {
                format!("{:.1} MB", i.size as f64 / 1024.0 / 1024.0)
            };
            preview.push(Line::from(vec![
                Span::raw("Type: "),
                Span::styled(
                    if i.is_dir { "Directory" } else { "File" },
                    Style::default().fg(Color::Yellow),
                ),
                Span::raw(" | Size: "),
                Span::styled(size_str, Style::default().fg(Color::Yellow)),
                Span::raw(" | Mod: "),
                Span::styled(&i.modified, Style::default().fg(Color::Yellow)),
            ]));
            preview.push(Line::from(""));
            if let Some(n) = &i.note {
                let (l, c) = if i.is_auto {
                    ("Auto", Color::Gray)
                } else {
                    ("Note", Color::Yellow)
                };
                preview.push(Line::from(vec![Span::styled(
                    format!("{}: ", l),
                    Style::default().fg(c).add_modifier(Modifier::BOLD),
                )]));
                preview.push(Line::from(Span::styled(
                    n,
                    Style::default().fg(Color::White),
                )));
                preview.push(Line::from(""));
            }

            if i.is_dir {
                let mut found = false;
                for name in &["README.md", "readme.md", "README.txt"] {
                    if let Ok(c) = fs::read_to_string(i.path.join(name)) {
                        preview.push(Line::from(Span::styled(
                            "README Preview:",
                            Style::default().fg(THEME_COLOR),
                        )));
                        preview.push(Line::from(""));
                        let ansi = render_markdown_to_ansi(&c, 80);
                        if let Ok(text) = ansi.into_text() {
                            for line in text.lines.iter().take(30) {
                                preview.push(line.clone());
                            }
                        }
                        if c.lines().count() > 30 {
                            preview.push(Line::from(Span::styled(
                                "...",
                                Style::default().fg(Color::DarkGray),
                            )));
                        }
                        found = true;
                        break;
                    }
                }
                if !found {
                    preview.push(Line::from(Span::styled(
                        "(No README found)",
                        Style::default().fg(Color::DarkGray),
                    )));
                }
            } else if let Ok(c) = fs::read_to_string(&i.path) {
                // Fix: collapsed else if
                let is_code = i.path.extension().is_some_and(|e| {
                    let s = e.to_string_lossy();
                    matches!(
                        s.as_ref(),
                        "rs" | "py"
                            | "js"
                            | "ts"
                            | "html"
                            | "css"
                            | "json"
                            | "toml"
                            | "yaml"
                            | "sh"
                            | "c"
                            | "cpp"
                            | "h"
                            | "java"
                            | "go"
                    )
                });
                if is_code {
                    preview.push(Line::from(Span::styled(
                        "Code Preview:",
                        Style::default().fg(THEME_COLOR),
                    )));
                    preview.push(Line::from(""));
                    let snippet: String = c.lines().take(40).collect::<Vec<_>>().join("\n");
                    let ansi = render_code_styled(&i.path, &snippet, true);
                    if let Ok(text) = ansi.into_text() {
                        for line in text.lines {
                            preview.push(line);
                        }
                    }
                } else {
                    preview.push(Line::from(Span::styled(
                        "Text Preview:",
                        Style::default().fg(THEME_COLOR),
                    )));
                    preview.push(Line::from(""));
                    for l in c.lines().take(30) {
                        preview.push(Line::from(l.to_string()));
                    }
                }
            } else {
                preview.push(Line::from(Span::styled(
                    "(Binary or Unreadable)",
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }
    }
    f.render_widget(
        Paragraph::new(preview)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(" Preview "),
            )
            .wrap(Wrap { trim: false }),
        main[1],
    );
    let footer_text = if let Some(idx) = app.state.selected() {
        format!(" {}/{} ", idx + 1, app.items.len())
    } else {
        " - ".into()
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " <q> Quit ",
                Style::default().bg(Color::Red).fg(Color::White),
            ),
            Span::styled(
                " <Enter> Read Full ",
                Style::default().bg(Color::Blue).fg(Color::White),
            ),
            Span::styled(
                " <Scroll/Keys> Navigate ",
                Style::default().bg(Color::DarkGray).fg(Color::White),
            ),
            Span::raw(footer_text),
        ])),
        chunks[2],
    );
}

fn main() {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("-h") | Some("--help") => {
            let green = "\x1b[32m";
            let cyan = "\x1b[36m";
            let yellow = "\x1b[33m";
            let bold = "\x1b[1m";
            let gray = "\x1b[90m";
            let reset = "\x1b[0m";
            println!("{}{}lsz v0.9.6{}", green, bold, reset);
            println!("  一个带有持久化注释与 Markdown 预览的现代化文件浏览工具。\n");
            println!("{}用法:{}", bold, reset);
            println!("  lsz [选项] [路径]\n");
            println!("{}核心选项:{}", bold, reset);
            print_row("[路径]", "列出指定路径下的文件（默认模式）", cyan, reset);
            print_row(
                "-i <路径>",
                "查看详情卡片 (支持 Markdown 渲染)",
                cyan,
                reset,
            );
            print_row("-l [路径]", "进入交互式 TUI (代码高亮预览)", cyan, reset);
            print_row("-s <内容> <路径>", "给指定文件/目录添加备注", cyan, reset);
            print_row("-d <路径>", "删除指定文件/目录的备注", cyan, reset);
            print_row("-gc", "清理数据库中已删除文件的记录", cyan, reset);
            print_row("-h, --help", "显示此帮助信息", cyan, reset);
            println!();
            println!("{}交互操作 (-l / -i):{}", bold, reset);
            print_row("↑ / k / 滚轮上", "向上移动 / 向上翻页", yellow, reset);
            print_row("↓ / j / 滚轮下", "向下移动 / 向下翻页", yellow, reset);
            print_row("Enter", "全屏阅读 (仅 -l 模式)", yellow, reset);
            print_row("q / Esc", "退出程序", yellow, reset);
            print_row("n", "切换行号 (仅代码阅读器)", yellow, reset);
            println!();
            println!(
                "{}💡 提示: 按住 Shift 键可使用鼠标划选复制文本{}",
                gray, reset
            );
            println!();
            let db_path = env::var("HOME")
                .map(|h| format!("{}/.lsz.db", h))
                .unwrap_or_else(|_| "未知".to_string());
            println!("{}数据存储: {}{}", gray, db_path, reset);
        }
        Some("-gc") => {
            let _ = gc_notes();
        }
        Some("-s") => {
            let n = args.next().expect("缺注释");
            let p = args.next().expect("缺路径");
            let _ = store_note(&p, &n);
        }
        Some("-d") => {
            let p = args.next().expect("缺路径");
            let _ = delete_note(&p);
        }
        Some("-i") => {
            let target = args.next().unwrap_or_else(|| ".".to_string());
            if let Err(e) = run_detail_card(target) {
                eprintln!("Error: {}", e);
            }
        }
        Some("-l") | Some("--list") => {
            let target = args.next().unwrap_or_else(|| ".".to_string());
            if let Err(e) = run_interactive_list(PathBuf::from(target)) {
                eprintln!("Error: {}", e);
            }
        }
        Some(target) if !target.starts_with('-') => {
            if let Err(e) = run_simple_list(target) {
                eprintln!("Error: {}", e);
            }
        }
        None => {
            if let Err(e) = run_simple_list(".") {
                eprintln!("Error: {}", e);
            }
        }
        Some(unk) => {
            eprintln!(
                "错误: 未知参数 '{}'\n请尝试使用 'lsz --help' 查看帮助。",
                unk
            );
        }
    }
}
