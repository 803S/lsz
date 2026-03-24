pub mod app;
pub mod ui;

use anyhow::{Result, anyhow};
use std::{fs, io::IsTerminal, path::PathBuf};

pub fn run(path: String) -> Result<()> {
    if !std::io::stdout().is_terminal() {
        return Err(anyhow!(
            "`lsz -l` 需要在 TTY 终端中运行；如果只想查看目录文本，请改用 `lsz [path]` 或 `lsz --plain [path]`"
        ));
    }
    let path = PathBuf::from(path);
    if !path.exists() {
        return Err(anyhow!("路径不存在: {}", path.display()));
    }
    let path = fs::canonicalize(path)?;
    let (cwd, initial_selection) = if path.is_file() {
        let parent = path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        (parent, Some(path))
    } else {
        (path, None)
    };
    app::event_loop::run(cwd, initial_selection)
}
