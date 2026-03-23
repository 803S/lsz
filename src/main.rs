// lsz.rs v0.9.7
// 依赖: ratatui 0.26, crossterm 0.27, termimad 0.26, syntect 5.0, ansi-to-tui 4.0, unicode-width 0.1, infer, sha2

mod analyzer;
mod archive;
mod imagepreview;
mod lscolors;
mod pdfpreview;

use analyzer::FileSecurityInfo;
use archive::ArchivePreview;
use imagepreview::ImageInfo;
use lscolors::LsColors;
use pdfpreview::PdfInfo;

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
    conn.execute("CREATE TABLE IF NOT EXISTS bookmarks (name TEXT PRIMARY KEY, path TEXT NOT NULL, note TEXT)", [])?;
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

fn add_bookmark(name: &str, path: &str, note: Option<&str>) -> Result<()> {
    let conn = open_db()?;
    let abs_path = fs::canonicalize(path)?;
    conn.execute(
        "INSERT OR REPLACE INTO bookmarks (name, path, note) VALUES (?1, ?2, ?3)",
        params![name, abs_path.to_string_lossy().to_string(), note],
    )?;
    println!("✓ 书签已添加: {} -> {}", name, abs_path.display());
    Ok(())
}

fn list_bookmarks() -> Result<()> {
    let conn = open_db()?;
    let mut stmt = conn.prepare("SELECT name, path, note FROM bookmarks ORDER BY name")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
        ))
    })?;

    let mut has_bookmarks = false;
    for row in rows {
        let (name, path, note) = row?;
        has_bookmarks = true;
        print!("\x1b[1;33m{:15}\x1b[0m -> {}", name, path);
        if let Some(n) = note {
            print!(" \x1b[90m— {}\x1b[0m", n);
        }
        println!();
    }

    if !has_bookmarks {
        println!("暂无书签，使用 \x1b[33mlsz -b add <名称> <路径>\x1b[0m 添加");
    }
    Ok(())
}

fn jump_to_bookmark(name: &str) -> Result<String> {
    let conn = open_db()?;
    let path: String = conn
        .query_row(
            "SELECT path FROM bookmarks WHERE name=?1",
            params![name],
            |r| r.get(0),
        )
        .map_err(|_| anyhow::anyhow!("未找到书签: {}", name))?;

    if !Path::new(&path).exists() {
        return Err(anyhow::anyhow!("书签路径已不存在: {}", path));
    }

    println!("{}", path);
    Ok(path)
}

fn delete_bookmark(name: &str) -> Result<()> {
    let conn = open_db()?;
    let count = conn.execute("DELETE FROM bookmarks WHERE name=?1", params![name])?;
    if count > 0 {
        println!("✓ 已删除书签: {}", name);
    } else {
        println!("! 未找到书签: {}", name);
    }
    Ok(())
}

fn run_simple_list(target: &str) -> Result<()> {
    let path = Path::new(target);
    if path.is_file() {
        println!("{} (文件)", target);
        return Ok(());
    }
    let conn = open_db()?;
    let ls_colors = LsColors::from_env();
    let mut entries = Vec::new();
    let mut max_name_len = 0;

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

            #[cfg(unix)]
            let is_executable = {
                use std::os::unix::fs::PermissionsExt;
                meta.permissions().mode() & 0o111 != 0
            };
            #[cfg(not(unix))]
            let is_executable = false;

            let is_symlink = meta.is_symlink();
            let is_dir = meta.is_dir();
            let color = ls_colors.for_path(&name, is_dir, is_symlink, is_executable);

            let name_len = name.chars().count();
            if name_len > max_name_len {
                max_name_len = name_len;
            }

            let suffix = if is_dir { "/" } else { "" };
            let display_name = format!("{}{}", name, suffix);

            entries.push((
                format!("  {}{}{}\x1b[0m", color, display_name, "\x1b[0m"),
                name_len + suffix.len(),
                note,
                is_auto,
            ));
        }
    }

    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    let target_width = max_name_len.max(20);

    for (disp, name_len, note, is_auto) in entries {
        if let Some(n) = note {
            let padding = " ".repeat(target_width.saturating_sub(name_len) + 4);
            let note_color = if is_auto { "\x1b[90m" } else { "\x1b[33m" };
            writeln!(lock, "{}{}{}— {} \x1b[0m", disp, padding, note_color, n)?;
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
    is_image: bool,
    is_pdf: bool,
    is_archive: bool,
    security_info: Option<FileSecurityInfo>,
    archive_preview: Option<ArchivePreview>,
    image_info: Option<ImageInfo>,
    pdf_info: Option<PdfInfo>,
}
struct App {
    items: Vec<FileEntry>,
    filtered_items: Vec<usize>,
    state: ListState,
    current_dir: PathBuf,
    preview_scroll: u16,
    dir_history: Vec<PathBuf>,
    is_searching: bool,
    search_input: String,
    is_search_input_mode: bool,
}

impl App {
    fn new(dir: PathBuf) -> Self {
        Self {
            items: Vec::new(),
            filtered_items: Vec::new(),
            state: ListState::default(),
            current_dir: dir,
            preview_scroll: 0,
            dir_history: Vec::new(),
            is_searching: false,
            search_input: String::new(),
            is_search_input_mode: false,
        }
    }

    fn visible_len(&self) -> usize {
        if self.is_searching {
            self.filtered_items.len()
        } else {
            self.items.len()
        }
    }

    fn selected_actual_index(&self) -> Option<usize> {
        let selected = self.state.selected()?;
        if self.is_searching {
            self.filtered_items.get(selected).copied()
        } else {
            Some(selected)
        }
    }

    fn selected_item_mut(&mut self) -> Option<&mut FileEntry> {
        let idx = self.selected_actual_index()?;
        self.items.get_mut(idx)
    }

    fn next(&mut self) {
        let len = self.visible_len();
        let i = match self.state.selected() {
            Some(i) if i + 1 < len => i + 1,
            _ => 0,
        };
        if len > 0 {
            self.state.select(Some(i));
            self.preview_scroll = 0;
        }
    }

    fn previous(&mut self) {
        let len = self.visible_len();
        let i = match self.state.selected() {
            Some(i) if i > 0 => i - 1,
            Some(_) => len.saturating_sub(1),
            None => 0,
        };
        if len > 0 {
            self.state.select(Some(i));
            self.preview_scroll = 0;
        }
    }

    fn apply_search(&mut self) {
        if self.search_input.is_empty() {
            self.is_searching = false;
            self.filtered_items.clear();
        } else {
            let query_lower = self.search_input.to_lowercase();
            self.filtered_items = self
                .items
                .iter()
                .enumerate()
                .filter(|(_, item)| item.name.to_lowercase().contains(&query_lower))
                .map(|(idx, _)| idx)
                .collect();
            self.is_searching = true;
        }
        if self.visible_len() > 0 {
            self.state.select(Some(0));
        } else {
            self.state.select(None);
        }
        self.preview_scroll = 0;
    }

    fn clear_search(&mut self) {
        self.search_input.clear();
        self.is_search_input_mode = false;
        self.apply_search();
    }

    fn ensure_selected_metadata(&mut self) {
        let Some(item) = self.selected_item_mut() else {
            return;
        };
        if item.is_dir {
            return;
        }
        if item.security_info.is_none() {
            item.security_info = analyzer::analyze_file(&item.path).ok();
        }
        if item.is_archive && item.archive_preview.is_none() && item.size <= 20 * 1024 * 1024 {
            item.archive_preview = archive::preview_archive(&item.path).ok();
        }
        if item.is_image && item.image_info.is_none() {
            item.image_info = imagepreview::get_image_info(&item.path);
        }
        if item.is_pdf && item.pdf_info.is_none() {
            item.pdf_info = pdfpreview::get_pdf_info(&item.path);
        }
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
                    let path = entry.path();
                    let is_image = meta.is_file() && imagepreview::is_image_file(&path);
                    let is_pdf = meta.is_file() && pdfpreview::is_pdf_file(&path);
                    let ext = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    let is_archive = meta.is_file()
                        && matches!(ext.as_str(), "zip" | "tar" | "gz" | "tgz" | "7z");
                    entries.push(FileEntry {
                        name,
                        path,
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
                        is_image,
                        is_pdf,
                        is_archive,
                        security_info: None,
                        archive_preview: None,
                        image_info: None,
                        pdf_info: None,
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
        self.filtered_items.clear();
        self.is_searching = false;
        self.search_input.clear();
        if !self.items.is_empty() {
            self.state.select(Some(0));
            self.ensure_selected_metadata();
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

// 代码阅读器 - 始终无边框，避免切换边框导致 ANSI 文本重排异常
fn run_code_viewer(path: &Path, content: String) -> Result<()> {
    let mut show_lines = true;
    let mut scroll_y = 0;

    let stdout = std::io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    loop {
        let ansi_text = render_code_styled(path, &content, show_lines);
        let text = ansi_text
            .into_text()
            .unwrap_or_else(|_| ratatui::text::Text::raw(&content));

        let _ = terminal.draw(|f| {
            let size = f.size();
            f.render_widget(Paragraph::new(text).scroll((scroll_y, 0)), size);
        })?;

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(k) if k.kind == KeyEventKind::Press => match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('n') => {
                        show_lines = !show_lines;
                        scroll_y = 0;
                    }
                    KeyCode::Down | KeyCode::Char('j') => scroll_y = scroll_y.saturating_add(1),
                    KeyCode::Up | KeyCode::Char('k') => scroll_y = scroll_y.saturating_sub(1),
                    KeyCode::PageDown => scroll_y = scroll_y.saturating_add(20),
                    KeyCode::PageUp => scroll_y = scroll_y.saturating_sub(20),
                    KeyCode::Home => scroll_y = 0,
                    KeyCode::End => scroll_y = u16::MAX,
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

fn run_pdf_viewer(path: &Path) -> Result<()> {
    let text = match pdfpreview::extract_pdf_text(path, 20) {
        Some(t) => t,
        None => {
            println!("无法提取PDF文本内容");
            return Ok(());
        }
    };

    let mut scroll_y: u16 = 0;
    let mut show_border = true;
    let lines: Vec<&str> = text.lines().collect();
    let total_lines = lines.len() as u16;

    enable_raw_mode()?;
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    loop {
        let _ = terminal.draw(|f| {
            let size = f.size();
            let title_str = path
                .file_name()
                .map(|n| format!(" PDF: {} | n: 切换边框 ", n.to_string_lossy()))
                .unwrap_or_else(|| " PDF Viewer ".to_string());

            let visible_lines: Vec<Line> = lines
                .iter()
                .skip(scroll_y as usize)
                .take(if show_border {
                    size.height as usize - 2
                } else {
                    size.height as usize
                })
                .map(|l| Line::from(l.to_string()))
                .collect();

            let paragraph = Paragraph::new(visible_lines).wrap(Wrap { trim: false });

            if show_border {
                f.render_widget(
                    paragraph.block(Block::default().borders(Borders::ALL).title(title_str)),
                    size,
                );
            } else {
                f.render_widget(paragraph, size);
            }
        })?;

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(k) if k.kind == KeyEventKind::Press => match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('n') => show_border = !show_border,
                    KeyCode::Down | KeyCode::Char('j') => {
                        scroll_y = scroll_y
                            .saturating_add(1)
                            .min(total_lines.saturating_sub(1))
                    }
                    KeyCode::Up | KeyCode::Char('k') => scroll_y = scroll_y.saturating_sub(1),
                    KeyCode::PageDown => {
                        scroll_y = (scroll_y + 20).min(total_lines.saturating_sub(1))
                    }
                    KeyCode::PageUp => scroll_y = scroll_y.saturating_sub(20),
                    _ => {}
                },
                Event::Mouse(m) => match m.kind {
                    MouseEventKind::ScrollDown => {
                        scroll_y = scroll_y
                            .saturating_add(3)
                            .min(total_lines.saturating_sub(1))
                    }
                    MouseEventKind::ScrollUp => scroll_y = scroll_y.saturating_sub(3),
                    _ => {}
                },
                _ => {}
            }
        }
    }

    Ok(())
}

fn open_archive_viewer(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    path: &Path,
    archive: &ArchivePreview,
) {
    let mut scroll_y: u16 = 0;
    let mut show_border = true;
    let total_lines: u16;

    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(vec![Span::styled(
        format!(
            "{} | Files: {} | Size: {}",
            archive.format, archive.total_files, archive.total_size
        ),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )]));

    if archive.is_supported {
        for (dir, files) in &archive.grouped {
            lines.push(Line::from(vec![Span::styled(
                format!("📂 {}", dir.trim_start_matches('/')),
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            )]));

            let display_files = if files.len() > 30 {
                &files[..10]
            } else {
                files
            };

            for entry in display_files {
                if entry.is_dir {
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled("📁 ", Style::default().fg(Color::Blue)),
                        Span::raw(&entry.name),
                        Span::raw("/"),
                    ]));
                } else {
                    lines.push(Line::from(vec![
                        Span::raw("    "),
                        Span::raw("  "),
                        Span::raw(&entry.name),
                        Span::styled(
                            format!("{:>10}", entry.size),
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]));
                }
            }

            if files.len() > 30 {
                lines.push(Line::from(vec![Span::styled(
                    format!("    ... and {} more files", files.len() - 10),
                    Style::default().fg(Color::DarkGray),
                )]));
            }
        }

        if archive.hidden_count > 0 {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                format!("... and {} more files (deep levels)", archive.hidden_count),
                Style::default().fg(Color::DarkGray),
            )]));
        }
    } else {
        lines.push(Line::from(vec![Span::styled(
            "需要 7z 命令支持",
            Style::default().fg(Color::Yellow),
        )]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        "按 q 退出 | n: 切换边框",
        Style::default().fg(Color::DarkGray),
    )]));

    total_lines = lines.len() as u16;

    loop {
        let _ = terminal.draw(|f| {
            let size = f.size();
            let paragraph = Paragraph::new(lines.clone())
                .wrap(Wrap { trim: false })
                .scroll((scroll_y, 0));

            if show_border {
                let title_str = path
                    .file_name()
                    .map(|n| format!(" Archive: {} | n: 切换边框 ", n.to_string_lossy()))
                    .unwrap_or_else(|| " Archive Preview ".to_string());
                f.render_widget(
                    paragraph.block(Block::default().borders(Borders::ALL).title(title_str)),
                    size,
                );
            } else {
                f.render_widget(paragraph, size);
            }
        });

        if event::poll(Duration::from_millis(100)).unwrap_or(false) {
            if let Ok(event) = event::read() {
                match event {
                    Event::Key(k) if k.kind == KeyEventKind::Press => match k.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Char('n') => show_border = !show_border,
                        KeyCode::Down | KeyCode::Char('j') => scroll_y = scroll_y.saturating_add(1),
                        KeyCode::Up | KeyCode::Char('k') => scroll_y = scroll_y.saturating_sub(1),
                        KeyCode::PageDown => {
                            scroll_y = (scroll_y + 20).min(total_lines.saturating_sub(1))
                        }
                        KeyCode::PageUp => scroll_y = scroll_y.saturating_sub(20),
                        KeyCode::Home => scroll_y = 0,
                        KeyCode::End => scroll_y = total_lines.saturating_sub(1),
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
        app.ensure_selected_metadata();
        terminal.draw(|f| ui_list(f, &mut app))?;

        if app.is_search_input_mode {
            terminal.draw(|f| {
                let size = f.size();
                let search_bar = Paragraph::new(vec![Line::from(vec![
                    Span::raw("/ "),
                    Span::styled(
                        &app.search_input,
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" (实时筛选, Enter确认, Esc退出)"),
                ])])
                .style(Style::default().bg(Color::Blue).fg(Color::White));
                f.render_widget(search_bar, size);
            })?;

            if let Ok(Event::Key(k)) = event::read() {
                if k.kind == KeyEventKind::Press {
                    match k.code {
                        KeyCode::Enter => {
                            app.is_search_input_mode = false;
                        }
                        KeyCode::Esc => {
                            app.clear_search();
                        }
                        KeyCode::Backspace => {
                            app.search_input.pop();
                            app.apply_search();
                        }
                        KeyCode::Down => app.next(),
                        KeyCode::Up => app.previous(),
                        KeyCode::Char('j') if app.search_input.is_empty() => app.next(),
                        KeyCode::Char('k') if app.search_input.is_empty() => app.previous(),
                        KeyCode::Char(c) => {
                            app.search_input.push(c);
                            app.apply_search();
                        }
                        _ => {}
                    }
                }
            }
            continue;
        }

        if event::poll(Duration::from_millis(200))? {
            match event::read()? {
                Event::Key(k) if k.kind == KeyEventKind::Press => match k.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Esc => {
                        if app.is_searching {
                            app.clear_search();
                        } else {
                            break;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => app.next(),
                    KeyCode::Up | KeyCode::Char('k') => app.previous(),
                    KeyCode::Char('/') => {
                        app.is_search_input_mode = true;
                    }
                    KeyCode::Char('b') => {
                        if let Some(idx) = app.state.selected() {
                            let item_opt = if app.is_searching {
                                app.filtered_items.get(idx).and_then(|&i| app.items.get(i))
                            } else {
                                app.items.get(idx)
                            };

                            if let Some(item) = item_opt {
                                if item.is_dir {
                                    let name = item.name.trim_end_matches('/');
                                    if let Err(e) =
                                        add_bookmark(name, item.path.to_str().unwrap_or(""), None)
                                    {
                                        eprintln!("Error: {}", e);
                                    }
                                }
                            }
                        }
                    }
                    KeyCode::Char('B') => {
                        drop(terminal);
                        disable_raw_mode()?;
                        execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
                        let _ = list_bookmarks();
                        println!("\n按书签名称跳转...");
                        let mut input = String::new();
                        if std::io::stdin().read_line(&mut input).is_ok() {
                            let name = input.trim();
                            if !name.is_empty() {
                                if let Ok(path) = jump_to_bookmark(name) {
                                    execute!(
                                        std::io::stdout(),
                                        EnterAlternateScreen,
                                        EnableMouseCapture
                                    )?;
                                    enable_raw_mode()?;
                                    let backend = CrosstermBackend::new(std::io::stdout());
                                    terminal = Terminal::new(backend)?;
                                    app = App::new(PathBuf::from(&path));
                                    if let Err(e) = app.load_files() {
                                        eprintln!("Error: {}", e);
                                        break;
                                    }
                                    continue;
                                }
                            }
                        }
                        execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
                        enable_raw_mode()?;
                        let backend = CrosstermBackend::new(std::io::stdout());
                        terminal = Terminal::new(backend)?;
                    }
                    KeyCode::Enter => {
                        if let Some(idx) = app.state.selected() {
                            let item_opt = if app.is_searching {
                                app.filtered_items.get(idx).and_then(|&i| app.items.get(i))
                            } else {
                                app.items.get(idx)
                            };

                            if let Some(item) = item_opt {
                                if item.is_dir {
                                    app.dir_history.push(app.current_dir.clone());
                                    let new_dir = item.path.clone();
                                    app = App::new(new_dir);
                                    if let Err(e) = app.load_files() {
                                        eprintln!("Error: {}", e);
                                        break;
                                    }
                                    app.preview_scroll = 0;
                                } else if item.image_info.is_some() {
                                    if let Ok(p) = fs::canonicalize(&item.path) {
                                        drop(terminal);
                                        disable_raw_mode()?;
                                        execute!(
                                            std::io::stdout(),
                                            LeaveAlternateScreen,
                                            DisableMouseCapture
                                        )?;
                                        let _ = imagepreview::run_image_fullscreen(&p);
                                        execute!(
                                            std::io::stdout(),
                                            EnterAlternateScreen,
                                            EnableMouseCapture
                                        )?;
                                        enable_raw_mode()?;
                                        let backend = CrosstermBackend::new(std::io::stdout());
                                        terminal = Terminal::new(backend)?;
                                        terminal.clear()?;
                                    }
                                } else if let Some(ref archive) = item.archive_preview {
                                    open_archive_viewer(&mut terminal, &item.path, archive);
                                    terminal.clear()?;
                                } else if item.pdf_info.is_some() {
                                    if let Ok(p) = fs::canonicalize(&item.path) {
                                        drop(terminal);
                                        disable_raw_mode()?;
                                        execute!(
                                            std::io::stdout(),
                                            LeaveAlternateScreen,
                                            DisableMouseCapture
                                        )?;
                                        if run_pdf_viewer(&p).is_err() {
                                            println!("\n无法在终端内提取 PDF 文本，改为系统默认程序打开...\n");
                                            let _ = pdfpreview::open_with_default_app(&p);
                                        }
                                        println!("\n按 Enter 返回...");
                                        let mut input = String::new();
                                        let _ = std::io::stdin().read_line(&mut input);
                                        execute!(
                                            std::io::stdout(),
                                            EnterAlternateScreen,
                                            EnableMouseCapture
                                        )?;
                                        enable_raw_mode()?;
                                        let backend = CrosstermBackend::new(std::io::stdout());
                                        terminal = Terminal::new(backend)?;
                                        terminal.clear()?;
                                    }
                                } else if pdfpreview::is_media_file(&item.path) {
                                    drop(terminal);
                                    disable_raw_mode()?;
                                    execute!(
                                        std::io::stdout(),
                                        LeaveAlternateScreen,
                                        DisableMouseCapture
                                    )?;
                                    println!("\n用系统默认程序打开...\n");
                                    if let Ok(p) = fs::canonicalize(&item.path) {
                                        let _ = pdfpreview::open_with_default_app(&p);
                                        println!("已在后台打开，请切换到对应的程序查看");
                                    }
                                    println!("按 Enter 返回...");
                                    let mut input = String::new();
                                    let _ = std::io::stdin().read_line(&mut input);
                                    execute!(
                                        std::io::stdout(),
                                        EnterAlternateScreen,
                                        EnableMouseCapture
                                    )?;
                                    enable_raw_mode()?;
                                    let backend = CrosstermBackend::new(std::io::stdout());
                                    terminal = Terminal::new(backend)?;
                                    terminal.clear()?;
                                } else if let Ok(p) = fs::canonicalize(&item.path) {
                                    if p.is_file() {
                                        let _ = open_full_screen_reader(&p);
                                        terminal.clear()?;
                                    }
                                }
                            }
                        }
                    }
                    KeyCode::Backspace | KeyCode::Left => {
                        if let Some(parent) = app.dir_history.pop() {
                            app = App::new(parent);
                            if let Err(e) = app.load_files() {
                                eprintln!("Error: {}", e);
                                break;
                            }
                            app.preview_scroll = 0;
                        } else if let Some(parent) = app.current_dir.parent() {
                            if parent.exists() {
                                app = App::new(parent.to_path_buf());
                                if let Err(e) = app.load_files() {
                                    eprintln!("Error: {}", e);
                                    break;
                                }
                                app.preview_scroll = 0;
                            }
                        }
                    }
                    KeyCode::Char('e') | KeyCode::Char('a') => {
                        let selected_path = if let Some(idx) = app.state.selected() {
                            let item_opt = if app.is_searching {
                                app.filtered_items.get(idx).and_then(|&i| app.items.get(i))
                            } else {
                                app.items.get(idx)
                            };
                            item_opt.map(|item| {
                                (item.path.clone(), item.name.clone(), item.note.clone())
                            })
                        } else {
                            None
                        };

                        if let Some((path, name, current_note)) = selected_path {
                            drop(terminal);
                            disable_raw_mode()?;
                            execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;

                            println!("\n\x1b[33m备注编辑\x1b[0m: {}", name);
                            println!(
                                "当前备注: {}\n",
                                current_note.clone().unwrap_or("(空)".to_string())
                            );
                            print!("输入新备注 (直接回车保持不变): ");
                            std::io::stdout().flush()?;

                            let mut input = String::new();
                            if std::io::stdin().read_line(&mut input).is_ok() {
                                let new_note = input.trim().to_string();
                                if !new_note.is_empty()
                                    && new_note != current_note.clone().unwrap_or_default()
                                {
                                    if let Err(e) =
                                        store_note(path.to_str().unwrap_or(""), &new_note)
                                    {
                                        eprintln!("保存失败: {}", e);
                                    } else {
                                        println!("\x1b[32m✓ 已保存备注\x1b[0m");
                                    }
                                }
                            }

                            execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
                            enable_raw_mode()?;
                            let backend = CrosstermBackend::new(std::io::stdout());
                            terminal = Terminal::new(backend)?;
                            if let Err(e) = app.load_files() {
                                eprintln!("Error: {}", e);
                                break;
                            }
                            if let Some(pos) = app.items.iter().position(|i| i.path == path) {
                                app.state.select(Some(pos));
                            }
                        }
                    }
                    _ => {}
                },
                Event::Mouse(m) => match m.kind {
                    MouseEventKind::ScrollDown => {
                        app.preview_scroll = app.preview_scroll.saturating_add(3)
                    }
                    MouseEventKind::ScrollUp => {
                        app.preview_scroll = app.preview_scroll.saturating_sub(3)
                    }
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

fn parse_ansi_color(code: &str) -> Color {
    let parts: Vec<&str> = code.split(';').collect();
    if parts.len() >= 1 {
        if let Ok(n) = parts[0].parse::<u8>() {
            if n == 1 && parts.len() >= 2 {
                if let Ok(color_num) = parts[1].parse::<u8>() {
                    return match color_num {
                        30 => Color::Black,
                        31 => Color::Red,
                        32 => Color::Green,
                        33 => Color::Yellow,
                        34 => Color::Blue,
                        35 => Color::Magenta,
                        36 => Color::Cyan,
                        37 => Color::White,
                        90..=97 => Color::Indexed(color_num - 90 + 8),
                        _ => Color::White,
                    };
                }
            }
            return match n {
                30 => Color::Black,
                31 => Color::Red,
                32 => Color::Green,
                33 => Color::Yellow,
                34 => Color::Blue,
                35 => Color::Magenta,
                36 => Color::Cyan,
                37 => Color::White,
                90..=97 => Color::Indexed(n - 90 + 8),
                _ => Color::White,
            };
        }
    }
    Color::White
}

fn get_tui_color(name: &str, is_dir: bool, is_symlink: bool, is_executable: bool) -> Color {
    let ls_colors = LsColors::from_env();
    let ansi_code = ls_colors.for_path(name, is_dir, is_symlink, is_executable);
    parse_ansi_color(&ansi_code)
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

    let search_indicator = if app.is_searching {
        format!(
            " 📁 {} [搜索: {}] ({} 结果)",
            app.current_dir.display(),
            app.search_input,
            app.filtered_items.len()
        )
    } else {
        format!(" 📁 {}", app.current_dir.display())
    };

    f.render_widget(
        Paragraph::new(search_indicator).style(
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

    let display_items: Vec<&FileEntry> = if app.is_searching {
        app.filtered_items
            .iter()
            .filter_map(|&idx| app.items.get(idx))
            .collect()
    } else {
        app.items.iter().collect()
    };

    let list_title = if app.is_searching {
        format!(
            " Files ({} / {}) ",
            app.filtered_items.len(),
            app.items.len()
        )
    } else {
        " Files ".to_string()
    };

    let items: Vec<ListItem> = display_items
        .iter()
        .map(|i| {
            let is_exec = i
                .security_info
                .as_ref()
                .map(|s| s.is_suspicious || s.file_signature.contains("Executable"))
                .unwrap_or(false);
            let suffix = if i.is_dir { "/" } else { "" };
            let color = get_tui_color(&i.name, i.is_dir, false, is_exec);
            ListItem::new(Line::from(vec![
                Span::styled(format!("{} {}", i.icon, i.name), Style::default().fg(color)),
                Span::raw(suffix),
            ]))
        })
        .collect();
    f.render_stateful_widget(
        List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(list_title),
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
        let item_opt = if app.is_searching {
            app.filtered_items.get(idx).and_then(|&i| app.items.get(i))
        } else {
            app.items.get(idx)
        };

        if let Some(i) = item_opt {
            let is_exec = i
                .security_info
                .as_ref()
                .map(|s| s.is_suspicious || s.file_signature.contains("Executable"))
                .unwrap_or(false);
            let suffix = if i.is_dir { "/" } else { "" };
            let color = get_tui_color(&i.name, i.is_dir, false, is_exec);
            preview.push(Line::from(vec![Span::styled(
                format!("{} {}", i.name, suffix),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
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

            if let Some(ref sec) = i.security_info {
                preview.push(Line::from(vec![
                    Span::raw("Real: "),
                    Span::styled(&sec.file_signature, Style::default().fg(Color::Cyan)),
                ]));
                preview.push(Line::from(vec![
                    Span::raw("MIME: "),
                    Span::styled(&sec.mime_type, Style::default().fg(Color::DarkGray)),
                    Span::raw(" | Ext: ."),
                    Span::styled(&sec.extension, Style::default().fg(Color::DarkGray)),
                ]));
                if sec.is_suspicious {
                    preview.push(Line::from(vec![Span::styled(
                        "  ⚠️  DANGER: Extension mismatch!",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    )]));
                }
                if let Some(ref hash) = sec.md5_hash {
                    preview.push(Line::from(vec![
                        Span::raw("MD5: "),
                        Span::styled(hash, Style::default().fg(Color::DarkGray)),
                    ]));
                } else {
                    preview.push(Line::from(Span::styled(
                        "MD5: <large file>",
                        Style::default().fg(Color::DarkGray),
                    )));
                }
            }

            if let Some(ref img) = i.image_info {
                preview.push(Line::from(""));
                preview.push(Line::from(vec![Span::styled(
                    "🖼 Image Info",
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                )]));
                preview.push(Line::from(vec![
                    Span::raw("Format: "),
                    Span::styled(&img.format, Style::default().fg(Color::Cyan)),
                    Span::raw(" | Size: "),
                    Span::styled(
                        format!("{}x{}", img.width, img.height),
                        Style::default().fg(Color::Yellow),
                    ),
                ]));
                preview.push(Line::from(vec![
                    Span::raw("Color: "),
                    Span::styled(&img.color_type, Style::default().fg(Color::DarkGray)),
                ]));
                preview.push(Line::from(vec![
                    Span::raw("Backend: "),
                    Span::styled(
                        imagepreview::backend_name(&img.backend),
                        Style::default().fg(Color::Cyan),
                    ),
                ]));
                preview.push(Line::from(vec![Span::styled(
                    "Enter: image preview / external viewer",
                    Style::default().fg(Color::Green),
                )]));

                if !img.exif_data.is_empty() {
                    preview.push(Line::from(Span::styled(
                        "─────────────────────────────",
                        Style::default().fg(Color::DarkGray),
                    )));
                    for (name, value) in &img.exif_data {
                        if value.len() < 40 {
                            preview.push(Line::from(vec![
                                Span::styled(
                                    format!("  {}: ", name),
                                    Style::default().fg(Color::DarkGray),
                                ),
                                Span::raw(value),
                            ]));
                        }
                    }
                }
            }

            if let Some(ref pdf) = i.pdf_info {
                preview.push(Line::from(""));
                preview.push(Line::from(vec![Span::styled(
                    "📄 PDF Document",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )]));
                preview.push(Line::from(vec![
                    Span::raw("Pages: "),
                    Span::styled(
                        format!("{}", pdf.page_count),
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::raw(" | Size: "),
                    Span::styled(
                        format!("{} B", pdf.file_size),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
                if !pdf.title.is_empty() {
                    preview.push(Line::from(vec![
                        Span::raw("Title: "),
                        Span::styled(&pdf.title, Style::default().fg(Color::Cyan)),
                    ]));
                }
                if !pdf.author.is_empty() {
                    preview.push(Line::from(vec![
                        Span::raw("Author: "),
                        Span::styled(&pdf.author, Style::default().fg(Color::Cyan)),
                    ]));
                }
                preview.push(Line::from(vec![Span::styled(
                    "Enter: terminal text view / external viewer",
                    Style::default().fg(Color::Green),
                )]));
            }

            if let Some(ref archive) = i.archive_preview {
                preview.push(Line::from(""));
                preview.push(Line::from(vec![
                    Span::styled(&archive.format, Style::default().fg(Color::Cyan)),
                    Span::raw(" | Files: "),
                    Span::styled(
                        format!("{}", archive.total_files),
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::raw(" | Size: "),
                    Span::styled(&archive.total_size, Style::default().fg(Color::Yellow)),
                ]));

                if archive.is_supported && !archive.grouped.is_empty() {
                    preview.push(Line::from(Span::styled(
                        "─────────────────────────────",
                        Style::default().fg(Color::DarkGray),
                    )));
                    for (dir, files) in &archive.grouped {
                        let dir_display = if dir.len() > 50 {
                            format!("...{}/", &dir[dir.len() - 47..])
                        } else {
                            dir.clone()
                        };
                        preview.push(Line::from(vec![
                            Span::styled("📂 ", Style::default().fg(Color::Blue)),
                            Span::styled(dir_display, Style::default().fg(Color::Cyan)),
                            Span::raw(" ("),
                            Span::styled(
                                format!("{}", files.len()),
                                Style::default().fg(Color::DarkGray),
                            ),
                            Span::raw(")"),
                        ]));
                        let display_files = if files.len() > 30 {
                            &files[..10]
                        } else {
                            files
                        };
                        for entry in display_files {
                            let name_display = if entry.name.len() > 45 {
                                format!("...{}", &entry.name[entry.name.len() - 42..])
                            } else {
                                entry.name.clone()
                            };
                            if entry.is_dir {
                                preview.push(Line::from(vec![
                                    Span::raw("    "),
                                    Span::styled("📁 ", Style::default().fg(Color::Blue)),
                                    Span::raw(name_display),
                                ]));
                            } else {
                                preview.push(Line::from(vec![
                                    Span::raw("    "),
                                    Span::raw("  "),
                                    Span::raw(name_display),
                                    Span::styled(
                                        format!("{:>10}", entry.size),
                                        Style::default().fg(Color::DarkGray),
                                    ),
                                ]));
                            }
                        }
                        if files.len() > 30 {
                            preview.push(Line::from(Span::styled(
                                "    ... and more files",
                                Style::default().fg(Color::DarkGray),
                            )));
                        }
                    }
                }

                if archive.hidden_count > 0 {
                    let more_msg = format!(
                        "  ... and {} more files (deep levels)",
                        archive.hidden_count
                    );
                    preview.push(Line::from(Span::styled(
                        more_msg,
                        Style::default().fg(Color::DarkGray),
                    )));
                }
                if archive.truncated {
                    preview.push(Line::from(Span::styled(
                        "  (预览已截断，按 Enter 查看更多)",
                        Style::default().fg(Color::DarkGray),
                    )));
                }

                if !archive.is_supported {
                    preview.push(Line::from(Span::styled(
                        "  (需要 7z 命令支持)",
                        Style::default().fg(Color::DarkGray),
                    )));
                }
            }
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
    let preview_widget = Paragraph::new(preview.clone())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Preview (Scroll to view) "),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.preview_scroll, 0));
    f.render_widget(preview_widget, main[1]);

    let item_count = if app.is_searching {
        app.filtered_items.len()
    } else {
        app.items.len()
    };
    let total_count = app.items.len();

    let footer_text = if app.is_searching {
        format!(
            " {}/{} (共{}个) ",
            app.state.selected().map(|i| i + 1).unwrap_or(0),
            item_count,
            total_count
        )
    } else {
        format!(
            " {}/{} ",
            app.state.selected().map(|i| i + 1).unwrap_or(0),
            total_count
        )
    };
    let can_go_back = !app.dir_history.is_empty() || app.current_dir.parent().is_some();

    let help_text = if let Some(idx) = app.state.selected() {
        let item_opt = if app.is_searching {
            app.filtered_items.get(idx).and_then(|&i| app.items.get(i))
        } else {
            app.items.get(idx)
        };

        if let Some(item) = item_opt {
            if item.image_info.is_some() {
                "Enter: 全屏查看图片 | /: 搜索"
            } else if item.pdf_info.is_some() || pdfpreview::is_media_file(&item.path) {
                "Enter: 用系统程序打开 | /: 搜索"
            } else if item.archive_preview.is_some() {
                "Enter: 查看压缩包内容 | /: 搜索"
            } else if item.is_dir {
                "Enter: 进入目录 | b: 收藏 | /: 搜索"
            } else {
                "Enter: 打开文件 | e: 备注 | /: 搜索"
            }
        } else {
            "↑↓: 选择 | /: 搜索 | q: 退出"
        }
    } else {
        "↑↓: 选择 | /: 搜索 | q: 退出"
    };

    let back_hint = if can_go_back { " ←:返回 |" } else { "" };
    let search_hint = if !app.is_searching {
        " /:搜索"
    } else {
        " ESC:取消搜索"
    };

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(back_hint, Style::default().fg(Color::White)),
            Span::styled(help_text, Style::default().fg(Color::Yellow)),
            Span::styled(search_hint, Style::default().fg(Color::Green)),
            Span::raw(format!("{:>15}", footer_text)),
        ])),
        chunks[2],
    );
}

fn main() {
    std::panic::set_hook(Box::new(|_| {
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture
        );
        let _ = crossterm::terminal::disable_raw_mode();
    }));

    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("-h") | Some("--help") => {
            let green = "\x1b[32m";
            let cyan = "\x1b[36m";
            let yellow = "\x1b[33m";
            let bold = "\x1b[1m";
            let gray = "\x1b[90m";
            let reset = "\x1b[0m";
            println!("{}{}lsz v0.9.7{}", green, bold, reset);
            println!("  一个带有持久化备注、实时搜索与懒加载预览的终端文件导航工具。\n");
            println!("{}用法:{}", bold, reset);
            println!("  lsz [选项] [路径]\n");
            println!("{}核心选项:{}", bold, reset);
            print_row("[路径]", "列出指定路径下的文件（默认模式）", cyan, reset);
            print_row("-l [路径]", "进入交互式 TUI (代码高亮预览)", cyan, reset);
            print_row(
                "-i <路径>",
                "查看详情卡片 (支持 Markdown 渲染)",
                cyan,
                reset,
            );
            print_row("-s <内容> <路径>", "给指定文件/目录添加备注", cyan, reset);
            print_row("-d <路径>", "删除指定文件/目录的备注", cyan, reset);
            print_row("-gc", "清理数据库中已删除文件的记录", cyan, reset);
            println!();
            println!("{}书签功能:{}", bold, reset);
            print_row("-b add <名称> <路径>", "添加书签", cyan, reset);
            print_row("-b add <名称>", "添加当前目录书签", cyan, reset);
            print_row("-b <名称>", "跳转到书签目录", cyan, reset);
            print_row("-b list", "查看所有书签", cyan, reset);
            print_row("-b del <名称>", "删除书签", cyan, reset);
            print_row("-h, --help", "显示此帮助信息", cyan, reset);
            println!();
            println!("{}交互操作 (-l 模式):{}", bold, reset);
            println!();
            println!("  {}基础导航{}", yellow, reset);
            print_row("↑ / ↓ / j / k", "上下选择文件", yellow, reset);
            print_row("← / Backspace", "返回上级目录", yellow, reset);
            print_row("Enter", "打开文件/进入目录", yellow, reset);
            println!();
            println!("  {}快捷操作{}", yellow, reset);
            print_row("/ (斜杠)", "进入类似 Vim 的实时搜索", yellow, reset);
            print_row("e", "编辑当前文件备注", yellow, reset);
            print_row("b", "收藏当前目录为书签", yellow, reset);
            print_row("B", "打开书签快速跳转", yellow, reset);
            println!();
            println!("  {}特殊预览{}", yellow, reset);
            print_row(
                "Enter",
                "图片: 终端适配预览/外部查看 | PDF: 文本阅读/外部查看",
                yellow,
                reset,
            );
            print_row("Enter", "压缩包: 查看内部文件", yellow, reset);
            print_row("滚轮", "滚动预览/列表", yellow, reset);
            println!();
            println!("  {}阅读器内部快捷键{}", yellow, reset);
            print_row(
                "n",
                "代码阅读: 切换行号；PDF/压缩包: 切换边框",
                yellow,
                reset,
            );
            print_row("q / Esc", "退出程序", yellow, reset);
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
        Some("-b") | Some("--bookmark") => match args.next().as_deref() {
            Some("list") | Some("ls") => {
                let _ = list_bookmarks();
            }
            Some("add") => {
                let name = args.next().expect("缺书签名称");
                let path = args.next().unwrap_or_else(|| ".".to_string());
                let _ = add_bookmark(&name, &path, None);
            }
            Some("del") | Some("delete") | Some("rm") => {
                let name = args.next().expect("缺书签名称");
                let _ = delete_bookmark(&name);
            }
            Some(name) => match jump_to_bookmark(name) {
                Ok(path) => println!("{}", path),
                Err(e) => eprintln!("Error: {}", e),
            },
            None => {
                let _ = list_bookmarks();
            }
        },
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
