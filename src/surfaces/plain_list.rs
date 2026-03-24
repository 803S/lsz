use crate::{
    domain::entry::EntryKind,
    infra::{db::Database, fs, lscolors::LsColors},
};
use anyhow::{Result, anyhow};
use std::{io::Write, path::PathBuf};
use unicode_width::UnicodeWidthStr;

pub fn run(path: &str) -> Result<()> {
    let path = PathBuf::from(path);
    if path.is_file() {
        println!("{}", path.display());
        return Ok(());
    }
    if !path.exists() {
        return Err(anyhow!("路径不存在: {}", path.display()));
    }
    let db = Database::open_best_effort()?;
    let entries = fs::load_entries(&path, false, &db)?;
    let ls_colors = LsColors::from_env();
    let mut stdout = std::io::stdout().lock();
    let width = entries
        .iter()
        .map(|entry| UnicodeWidthStr::width(entry.display_name().as_str()))
        .max()
        .unwrap_or(0)
        .max(20);
    for entry in entries {
        let color = ls_colors.for_path(
            &entry.name,
            matches!(entry.kind, EntryKind::Directory),
            entry.is_symlink,
            fs::is_executable(&entry.path),
        );
        let display_name = entry.display_name();
        let name_width = UnicodeWidthStr::width(display_name.as_str());
        let padding = " ".repeat(width.saturating_sub(name_width) + 4);
        if let Some(note) = entry.note.as_ref() {
            writeln!(
                stdout,
                "{}{}\x1b[0m{}\x1b[33m# {}\x1b[0m",
                color,
                display_name,
                padding,
                compact_suffix(note),
            )?;
        } else if let Some(summary) = entry.auto_summary.as_ref() {
            writeln!(
                stdout,
                "{}{}\x1b[0m{}\x1b[90m# {}\x1b[0m",
                color,
                display_name,
                padding,
                compact_suffix(summary),
            )?;
        } else {
            writeln!(stdout, "{}{}\x1b[0m", color, display_name)?;
        }
    }
    Ok(())
}

fn compact_suffix(text: &str) -> String {
    fs::truncate_chars(&text.split_whitespace().collect::<Vec<_>>().join(" "), 72)
}
