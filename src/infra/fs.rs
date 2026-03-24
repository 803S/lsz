use crate::{
    domain::entry::{EntryItem, EntryKind},
    infra::db::Database,
};
use anyhow::Result;
use chrono::{DateTime, Local};
use std::{
    fs::{self, File},
    io::{BufRead, BufReader},
    os::unix::fs::PermissionsExt,
    path::Path,
    time::SystemTime,
};

pub fn load_entries(path: &Path, include_hidden: bool, db: &Database) -> Result<Vec<EntryItem>> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let is_hidden = name.starts_with('.');
        if is_hidden && !include_hidden {
            continue;
        }
        let meta = fs::symlink_metadata(&path)?;
        let file_type = meta.file_type();
        let is_symlink = file_type.is_symlink();
        let canonical = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        let target_meta = fs::metadata(&path).unwrap_or(meta.clone());
        let note = db.resolve_note(&canonical).ok().flatten();
        let kind = detect_kind(&path, &target_meta);
        let auto_summary = if matches!(kind, EntryKind::Directory) {
            auto_summary(&canonical)
        } else {
            None
        };
        entries.push(EntryItem {
            path: canonical,
            name,
            kind,
            is_hidden,
            is_symlink,
            size: Some(target_meta.len()),
            mtime: target_meta.modified().ok(),
            note,
            auto_summary,
        });
    }
    entries.sort_by(|a, b| match (&a.kind, &b.kind) {
        (EntryKind::Directory, EntryKind::Directory) | (_, _) if a.kind == b.kind => {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        }
        (EntryKind::Directory, _) => std::cmp::Ordering::Less,
        (_, EntryKind::Directory) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok(entries)
}

pub fn detect_kind(path: &Path, meta: &fs::Metadata) -> EntryKind {
    if meta.is_dir() {
        return EntryKind::Directory;
    }
    if !meta.is_file() {
        return EntryKind::Unknown;
    }
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match file_name.as_str() {
        "dockerfile"
        | "makefile"
        | "justfile"
        | ".env"
        | ".gitignore"
        | ".dockerignore"
        | "nginx.conf"
        | "requirements.txt"
        | "gemfile"
        | "rakefile"
        | "brewfile"
        | "cmakelists.txt"
        | ".bashrc"
        | ".zshrc"
        | ".profile"
        | ".bash_profile"
        | ".bash_aliases"
        | "powershell_profile.ps1" => return EntryKind::Code,
        "readme.md" => return EntryKind::Markdown,
        "readme.txt" => return EntryKind::Text,
        _ => {}
    }
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "md" | "markdown" => EntryKind::Markdown,
        "rs" | "py" | "js" | "ts" | "tsx" | "jsx" | "toml" | "json" | "yaml" | "yml" | "sh"
        | "zsh" | "bash" | "fish" | "go" | "c" | "cpp" | "cc" | "cxx" | "h" | "hh" | "hpp"
        | "hxx" | "java" | "css" | "html" | "sql" | "lua" | "vim" | "xml" | "nix" | "proto"
        | "properties" | "cfg" | "ini" | "conf" | "env" | "php" | "phtml" | "rb" | "ru"
        | "gemspec" | "ps1" | "psm1" | "psd1" | "bat" | "cmd" | "swift" | "kt" | "kts" | "cs"
        | "m" | "mm" => EntryKind::Code,
        "txt" | "log" => EntryKind::Text,
        "zip" | "tar" | "gz" | "tgz" | "7z" => EntryKind::Archive,
        "pdf" => EntryKind::Pdf,
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg" | "ico" | "tif" | "tiff" => {
            EntryKind::Image
        }
        _ => {
            if has_script_shebang(path) {
                EntryKind::Code
            } else if is_probably_text(path) {
                EntryKind::Text
            } else {
                EntryKind::Binary
            }
        }
    }
}

pub fn find_project_doc(dir: &Path) -> Option<std::path::PathBuf> {
    for candidate in project_doc_candidates() {
        let candidate = dir.join(candidate);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub fn is_probably_text(path: &Path) -> bool {
    let Ok(bytes) = fs::read(path) else {
        return false;
    };
    let sample = &bytes[..bytes.len().min(2048)];
    sample
        .iter()
        .all(|byte| byte.is_ascii_graphic() || byte.is_ascii_whitespace())
}

fn has_script_shebang(path: &Path) -> bool {
    let Ok(file) = File::open(path) else {
        return false;
    };
    let mut first_line = String::new();
    if BufReader::new(file)
        .read_line(&mut first_line)
        .ok()
        .unwrap_or(0)
        == 0
    {
        return false;
    }
    let first_line = first_line.trim();
    first_line.starts_with("#!")
        && [
            "sh",
            "bash",
            "zsh",
            "fish",
            "python",
            "ruby",
            "php",
            "pwsh",
            "powershell",
        ]
        .iter()
        .any(|needle| first_line.contains(needle))
}

pub fn auto_summary(dir: &Path) -> Option<String> {
    if let Some(doc) = find_project_doc(dir) {
        if let Ok(file) = File::open(doc) {
            let mut line = String::new();
            if BufReader::new(file).read_line(&mut line).ok()? > 0 {
                let line = line.trim().trim_start_matches(['#', ' ']).to_string();
                if !line.is_empty() {
                    return Some(truncate_chars(&line, 48));
                }
            }
        }
    }
    if let Ok(file) = File::open(dir.join("Cargo.toml")) {
        for line in BufReader::new(file).lines().map_while(Result::ok).take(20) {
            if line.starts_with("description") {
                if let Some(value) = line.split('=').nth(1) {
                    return Some(truncate_chars(value.trim().trim_matches(['"', '\'']), 48));
                }
            }
        }
    }
    None
}

fn project_doc_candidates() -> &'static [&'static str] {
    &[
        "README.md",
        "readme.md",
        "README.txt",
        "AGENTS.md",
        "agents.md",
        "AGENT.md",
        "agent.md",
        "GUIDE.md",
        "guide.md",
        "INTRO.md",
        "intro.md",
        "OVERVIEW.md",
        "overview.md",
        "CONTRIBUTING.md",
        "contributing.md",
        "docs/README.md",
        "docs/readme.md",
    ]
}

pub fn truncate_chars(input: &str, max: usize) -> String {
    let len = input.chars().count();
    if len <= max {
        input.to_string()
    } else {
        format!(
            "{}...",
            input
                .chars()
                .take(max.saturating_sub(3))
                .collect::<String>()
        )
    }
}

pub fn size_label(size: Option<u64>) -> String {
    match size.unwrap_or(0) {
        value if value < 1024 => format!("{value} B"),
        value if value < 1024 * 1024 => format!("{:.1} KB", value as f64 / 1024.0),
        value if value < 1024 * 1024 * 1024 => format!("{:.1} MB", value as f64 / 1024.0 / 1024.0),
        value => format!("{:.1} GB", value as f64 / 1024.0 / 1024.0 / 1024.0),
    }
}

pub fn time_label(time: Option<SystemTime>) -> String {
    time.map(|value| {
        let value: DateTime<Local> = value.into();
        value.format("%Y-%m-%d %H:%M").to_string()
    })
    .unwrap_or_else(|| "-".to_string())
}

pub fn is_executable(path: &Path) -> bool {
    fs::metadata(path)
        .map(|meta| meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs::{self, File},
        time::{SystemTime, UNIX_EPOCH},
    };

    fn unique_path(name: &str, ext: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("lsz-fs-test-{name}-{nanos}.{ext}"))
    }

    #[test]
    fn detect_kind_classifies_markdown_and_code() {
        let markdown = unique_path("demo", "md");
        let code = unique_path("demo", "rs");
        fs::write(&markdown, "# 标题\n").expect("write markdown");
        fs::write(&code, "fn main() {}\n").expect("write code");

        let markdown_meta = fs::metadata(&markdown).expect("markdown meta");
        let code_meta = fs::metadata(&code).expect("code meta");

        assert_eq!(detect_kind(&markdown, &markdown_meta), EntryKind::Markdown);
        assert_eq!(detect_kind(&code, &code_meta), EntryKind::Code);

        fs::remove_file(markdown).expect("cleanup markdown");
        fs::remove_file(code).expect("cleanup code");
    }

    #[test]
    fn detect_kind_supports_common_named_config_files() {
        let dir = std::env::temp_dir().join(format!(
            "lsz-fs-name-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create dir");
        let dockerfile = dir.join("Dockerfile");
        let env_file = dir.join(".env");
        fs::write(&dockerfile, "FROM rust:latest\n").expect("write dockerfile");
        fs::write(&env_file, "KEY=value\n").expect("write env file");

        let docker_meta = fs::metadata(&dockerfile).expect("docker meta");
        let env_meta = fs::metadata(&env_file).expect("env meta");

        assert_eq!(detect_kind(&dockerfile, &docker_meta), EntryKind::Code);
        assert_eq!(detect_kind(&env_file, &env_meta), EntryKind::Code);

        fs::remove_file(dockerfile).expect("cleanup dockerfile");
        fs::remove_file(env_file).expect("cleanup env");
        fs::remove_dir_all(dir).expect("cleanup dir");
    }

    #[test]
    fn detect_kind_supports_more_code_languages() {
        let dir = std::env::temp_dir().join(format!(
            "lsz-fs-lang-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create dir");
        let php = dir.join("demo.php");
        let ruby = dir.join("demo.rb");
        let powershell = dir.join("demo.ps1");
        fs::write(&php, "<?php echo 1;").expect("write php");
        fs::write(&ruby, "puts 'hi'\n").expect("write ruby");
        fs::write(&powershell, "Write-Host 'hi'\n").expect("write powershell");

        assert_eq!(
            detect_kind(&php, &fs::metadata(&php).expect("php meta")),
            EntryKind::Code
        );
        assert_eq!(
            detect_kind(&ruby, &fs::metadata(&ruby).expect("ruby meta")),
            EntryKind::Code
        );
        assert_eq!(
            detect_kind(
                &powershell,
                &fs::metadata(&powershell).expect("powershell meta")
            ),
            EntryKind::Code
        );

        fs::remove_file(php).expect("cleanup php");
        fs::remove_file(ruby).expect("cleanup ruby");
        fs::remove_file(powershell).expect("cleanup powershell");
        fs::remove_dir_all(dir).expect("cleanup dir");
    }

    #[test]
    fn find_project_doc_prefers_readme_then_agents() {
        let dir = std::env::temp_dir().join(format!(
            "lsz-fs-doc-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create dir");
        let agents = dir.join("AGENTS.md");
        let readme = dir.join("README.md");

        fs::write(&agents, "# Agents\n").expect("write agents");
        assert_eq!(find_project_doc(&dir).as_deref(), Some(agents.as_path()));

        fs::write(&readme, "# Readme\n").expect("write readme");
        assert_eq!(find_project_doc(&dir).as_deref(), Some(readme.as_path()));

        fs::remove_file(readme).expect("cleanup readme");
        fs::remove_file(agents).expect("cleanup agents");
        fs::remove_dir_all(dir).expect("cleanup dir");
    }

    #[test]
    fn detect_kind_recognizes_shebang_script_as_code() {
        let path = unique_path("script", "tmp");
        fs::write(&path, "#!/usr/bin/env bash\necho hi\n").expect("write script");

        let kind = detect_kind(&path, &fs::metadata(&path).expect("script meta"));
        assert_eq!(kind, EntryKind::Code);

        fs::remove_file(path).expect("cleanup script");
    }

    #[cfg(unix)]
    #[test]
    fn load_entries_keeps_symlink_information() {
        use std::os::unix::fs::symlink;

        let dir = std::env::temp_dir().join(format!(
            "lsz-fs-dir-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create dir");
        let target = dir.join("target.txt");
        let link = dir.join("target-link.txt");
        File::create(&target).expect("create target");
        symlink(&target, &link).expect("create symlink");

        let db = Database::open_ephemeral().expect("db");
        let entries = load_entries(&dir, true, &db).expect("load entries");
        let link_entry = entries
            .iter()
            .find(|entry| entry.name == "target-link.txt")
            .expect("link entry");
        assert!(link_entry.is_symlink);

        fs::remove_file(link).expect("cleanup link");
        fs::remove_file(target).expect("cleanup target");
        fs::remove_dir_all(dir).expect("cleanup dir");
    }
}
