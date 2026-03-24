mod cli;
mod config;
mod domain;
mod infra;
mod surfaces;

use anyhow::Result;
use cli::args::{BookmarkCommand, CliCommand, NoteCommand, parse_args};
use std::{env, io::IsTerminal, path::PathBuf};

fn main() {
    if let Err(err) = run() {
        eprintln!("错误: {err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    match parse_args(env::args().skip(1))? {
        CliCommand::Help => {
            print_help();
            Ok(())
        }
        CliCommand::HelpKeys => {
            config::keymap::print_help_keys();
            Ok(())
        }
        CliCommand::Plain { path } => surfaces::plain_list::run(&path),
        CliCommand::Inspect { path } => surfaces::detail_card::run(&path),
        CliCommand::Tui { path } => surfaces::tui::run(path),
        CliCommand::Note(cmd) => match cmd {
            NoteCommand::Set { path, note } => {
                let path_buf = PathBuf::from(path);
                let display_path =
                    std::fs::canonicalize(&path_buf).unwrap_or_else(|_| path_buf.clone());
                infra::db::Database::open_default()?.save_note(path_buf.as_path(), &note)?;
                println!("已保存备注: {}", display_path.display());
                Ok(())
            }
            NoteCommand::Delete { path } => {
                let path_buf = PathBuf::from(path);
                let display_path =
                    std::fs::canonicalize(&path_buf).unwrap_or_else(|_| path_buf.clone());
                infra::db::Database::open_default()?.delete_note(path_buf.as_path())?;
                println!("已删除备注: {}", display_path.display());
                Ok(())
            }
            NoteCommand::Gc => {
                let deleted = infra::db::Database::open_default()?.gc_notes()?;
                println!("已清理 {deleted} 条备注记录");
                Ok(())
            }
        },
        CliCommand::Bookmark(cmd) => match cmd {
            BookmarkCommand::Add { name, path } => {
                let path_buf = PathBuf::from(path);
                let display_path =
                    std::fs::canonicalize(&path_buf).unwrap_or_else(|_| path_buf.clone());
                infra::db::Database::open_default()?.add_bookmark(
                    &name,
                    path_buf.as_path(),
                    None,
                )?;
                println!("已保存书签 {name} -> {}", display_path.display());
                Ok(())
            }
            BookmarkCommand::Delete { name } => {
                infra::db::Database::open_default()?.delete_bookmark(&name)?;
                println!("已删除书签 {name}");
                Ok(())
            }
            BookmarkCommand::List => {
                for bookmark in infra::db::Database::open_default()?.list_bookmarks()? {
                    if let Some(note) = bookmark.note {
                        println!(
                            "{:16} -> {}  ({note})",
                            bookmark.name,
                            bookmark.path.display()
                        );
                    } else {
                        println!("{:16} -> {}", bookmark.name, bookmark.path.display());
                    }
                }
                Ok(())
            }
            BookmarkCommand::Jump { name } => {
                let path = infra::db::Database::open_default()?.bookmark_path(&name)?;
                println!("{}", path.display());
                Ok(())
            }
        },
    }
}

fn print_help() {
    let use_color = std::io::stdout().is_terminal();
    let db_path = infra::db::default_db_path().ok();
    let paint = |text: &str, code: &str| {
        if use_color {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    };
    let title = |text: &str| paint(text, "1;36");
    let section = |text: &str| paint(text, "1;33");
    let command = |text: &str| paint(text, "32");
    let dim = |text: &str| paint(text, "2");

    println!("{}", title("lsz"));
    println!();
    println!(
        "{}",
        dim("围绕注释、收藏和快速了解 path / 目录内容设计的终端工具。")
    );
    println!();
    println!("{}", section("常用入口"));
    println!(
        "  {}       轻量列表，顺手看备注和摘要",
        command("lsz [path]")
    );
    println!(
        "  {}  纯文本列表，适合 grep / fzf / 重定向",
        command("lsz --plain [path]")
    );
    println!("  {}    项目卡片 / 说明文档预览", command("lsz -i [path]"));
    println!("  {}    TUI 浏览、搜索、阅读", command("lsz -l [path]"));
    println!("  {}  TUI 全量快捷键", command("lsz --help-keys"));
    println!();
    println!("{}", section("备注与书签"));
    println!("  {}  添加备注", command("lsz -s <内容> <path>"));
    println!("  {}       删除备注", command("lsz -d <path>"));
    println!("  {}           清理失效备注", command("lsz -gc"));
    println!("  {}  添加书签", command("lsz -b add <name> [path]"));
    println!("  {}            输出书签 path", command("lsz -b <name>"));
    println!("  {}          查看书签列表", command("lsz -b list"));
    println!("  {}     删除书签", command("lsz -b del <name>"));
    println!();
    println!("{}", section("TUI 常用"));
    println!(
        "  {} 帮助    {} 搜索    {} 命令    {} 阅读",
        command("? / F1"),
        command("/"),
        command(":"),
        command("Enter")
    );
    println!("  {} 切换行号    {} 外部打开", command("n"), command("o"));
    println!();
    println!("{}", section("TUI 里按 : 可用"));
    println!("  {}           打开帮助", command("help"));
    println!(
        "  {} 或 {}   按名称 / 修改时间排序",
        command("sort name"),
        command("sort mtime")
    );
    println!("  {}  保存当前目录为书签", command("bookmark add [name]"));
    println!("  {}   跳到已有书签", command("bookmark jump <name>"));
    println!(
        "  {} 或 {}  编辑 / 清空当前项备注",
        command("note edit"),
        command("note clear")
    );
    println!(
        "  {} 或 {}  打开阅读器 / 交给系统打开",
        command("inspect"),
        command("open external")
    );
    println!();
    println!("{}", section("示例"));
    println!("  {}", command("lsz"));
    println!("  {}", command("lsz -i ."));
    println!("  {}", command("lsz -l ."));
    if let Some(db_path) = db_path {
        println!();
        println!("{}", dim(&format!("数据存储位置：{}", db_path.display())));
    }
}
