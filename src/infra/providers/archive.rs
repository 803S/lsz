use crate::domain::{
    entry::EntryKind,
    preview::{PreviewKind, PreviewLine, PreviewResult, PreviewWrapMode},
};
use anyhow::{Result, anyhow};
use std::{collections::BTreeMap, path::Path, process::Command};

use super::{PreviewProvider, PreviewRequest};

const MAX_DEPTH: usize = 3;
const MAX_ENTRIES: usize = 1000;
const PER_DIR_COLLAPSE_THRESHOLD: usize = 50;
const PER_DIR_SHOW_LIMIT: usize = 10;

pub struct ArchiveProvider;

#[derive(Debug, Clone)]
struct ArchiveEntry {
    path: String,
    size: Option<u64>,
    is_dir: bool,
}

#[derive(Debug, Default, Clone)]
struct TreeNode {
    is_dir: bool,
    size: Option<u64>,
    children: BTreeMap<String, TreeNode>,
}

#[derive(Debug, Clone)]
struct ArchivePreviewData {
    format: String,
    total_entries: usize,
    total_size: Option<u64>,
    lines: Vec<PreviewLine>,
    warnings: Vec<String>,
}

impl PreviewProvider for ArchiveProvider {
    fn supports(&self, request: &PreviewRequest) -> bool {
        matches!(request.kind, EntryKind::Archive)
    }

    fn load(&self, request: &PreviewRequest) -> Result<PreviewResult> {
        let path = &request.path;
        let preview = preview_archive(path, request.viewport_hint.max(60))
            .map_err(|error| anyhow!("当前环境无法生成压缩包树形预览: {error}"))?;

        let mut metadata = vec![
            ("格式".to_string(), preview.format),
            ("条目".to_string(), preview.total_entries.to_string()),
        ];
        if let Some(total_size) = preview.total_size {
            metadata.push(("总大小".to_string(), format_size(total_size)));
        }

        Ok(PreviewResult {
            kind: PreviewKind::Archive,
            title: title_for(path),
            subtitle: Some(path.display().to_string()),
            provider_name: "压缩包".to_string(),
            degraded: !preview.warnings.is_empty(),
            metadata,
            warnings: preview.warnings,
            actions: vec!["o：外部打开".to_string()],
            wrap_mode: PreviewWrapMode::NoWrap,
            lines: preview.lines,
            first_line_number: 1,
        })
    }
}

fn preview_archive(path: &Path, max_lines: usize) -> std::io::Result<ArchivePreviewData> {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "zip" => preview_zip(path, max_lines),
        "tar" | "gz" | "tgz" => preview_tar_like(path, max_lines),
        "7z" => preview_7z(path, max_lines),
        _ => Err(std::io::Error::other("unsupported archive format")),
    }
}

fn preview_zip(path: &Path, max_lines: usize) -> std::io::Result<ArchivePreviewData> {
    let output = Command::new("zipinfo")
        .args(["-l", &path.to_string_lossy()])
        .output()?;
    if !output.status.success() {
        return Err(std::io::Error::other("zipinfo failed"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();
    for line in stdout.lines().skip(3) {
        if line.contains(" files") && line.contains(" bytes") {
            break;
        }
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 10 {
            continue;
        }
        let size = cols[3].parse::<u64>().ok();
        let name = cols[9..].join(" ");
        let is_dir = name.ends_with('/');
        entries.push(ArchiveEntry {
            path: name,
            size,
            is_dir,
        });
        if entries.len() >= MAX_ENTRIES {
            break;
        }
    }

    Ok(build_archive_preview(
        "ZIP".to_string(),
        entries,
        max_lines,
        true,
    ))
}

fn preview_tar_like(path: &Path, max_lines: usize) -> std::io::Result<ArchivePreviewData> {
    let output = Command::new("tar")
        .args(["-tf", &path.to_string_lossy()])
        .output()?;
    if !output.status.success() {
        return Err(std::io::Error::other("tar list failed"));
    }

    let entries = String::from_utf8_lossy(&output.stdout)
        .lines()
        .take(MAX_ENTRIES)
        .map(|line| ArchiveEntry {
            path: line.to_string(),
            size: None,
            is_dir: line.ends_with('/'),
        })
        .collect::<Vec<_>>();

    Ok(build_archive_preview(
        "TAR".to_string(),
        entries,
        max_lines,
        false,
    ))
}

fn preview_7z(path: &Path, max_lines: usize) -> std::io::Result<ArchivePreviewData> {
    let output = Command::new("7z")
        .args(["l", &path.to_string_lossy()])
        .output()?;
    if !output.status.success() {
        return Err(std::io::Error::other("7z list failed"));
    }

    let mut entries = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if !line.contains(' ') || line.starts_with("----") {
            continue;
        }
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 6 {
            continue;
        }
        let Ok(size) = cols[3].parse::<u64>() else {
            continue;
        };
        let path = cols[5..].join(" ");
        entries.push(ArchiveEntry {
            is_dir: false,
            path,
            size: Some(size),
        });
        if entries.len() >= MAX_ENTRIES {
            break;
        }
    }

    Ok(build_archive_preview(
        "7Z".to_string(),
        entries,
        max_lines,
        true,
    ))
}

fn build_archive_preview(
    format: String,
    entries: Vec<ArchiveEntry>,
    max_lines: usize,
    has_sizes: bool,
) -> ArchivePreviewData {
    let total_entries = entries.len();
    let total_size = has_sizes.then(|| entries.iter().filter_map(|entry| entry.size).sum());
    let (lines, depth_truncated, breadth_collapsed) = build_tree_lines(&entries, max_lines);
    let mut warnings = Vec::new();
    if total_entries >= MAX_ENTRIES {
        warnings.push(format!("仅展示前 {MAX_ENTRIES} 条压缩包条目"));
    }
    if depth_truncated {
        warnings.push(format!("树形预览最多展开 {MAX_DEPTH} 层"));
    }
    if breadth_collapsed {
        warnings.push(format!(
            "单层目录超过 {PER_DIR_COLLAPSE_THRESHOLD} 条时，仅展示前 {PER_DIR_SHOW_LIMIT} 条"
        ));
    }

    ArchivePreviewData {
        format,
        total_entries,
        total_size,
        lines,
        warnings,
    }
}

fn build_tree_lines(entries: &[ArchiveEntry], max_lines: usize) -> (Vec<PreviewLine>, bool, bool) {
    let mut root = TreeNode::default();
    for entry in entries {
        insert_tree_entry(&mut root, entry);
    }

    let mut lines = vec![PreviewLine::plain("./")];
    let mut depth_truncated = false;
    let mut breadth_collapsed = false;
    render_tree_children(
        &root,
        "",
        0,
        max_lines.saturating_sub(1),
        &mut lines,
        &mut depth_truncated,
        &mut breadth_collapsed,
    );

    if lines.len() == 1 {
        lines.push(PreviewLine::plain("（空压缩包）"));
    }
    (lines, depth_truncated, breadth_collapsed)
}

fn insert_tree_entry(root: &mut TreeNode, entry: &ArchiveEntry) {
    let components = entry
        .path
        .trim_matches('/')
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if components.is_empty() {
        return;
    }

    let mut node = root;
    for (index, component) in components.iter().enumerate() {
        let is_last = index + 1 == components.len();
        node = node.children.entry((*component).to_string()).or_default();
        if !is_last {
            node.is_dir = true;
            continue;
        }
        node.is_dir = entry.is_dir || entry.path.ends_with('/');
        node.size = entry.size;
    }
}

fn render_tree_children(
    node: &TreeNode,
    prefix: &str,
    depth: usize,
    max_lines: usize,
    lines: &mut Vec<PreviewLine>,
    depth_truncated: &mut bool,
    breadth_collapsed: &mut bool,
) {
    let children = node.children.iter().collect::<Vec<_>>();
    let collapse = children.len() > PER_DIR_COLLAPSE_THRESHOLD;
    let visible_children = if collapse {
        *breadth_collapsed = true;
        children
            .into_iter()
            .take(PER_DIR_SHOW_LIMIT)
            .collect::<Vec<_>>()
    } else {
        children
    };

    for (index, (name, child)) in visible_children.iter().enumerate() {
        if lines.len() >= max_lines + 1 {
            return;
        }

        let is_last = index + 1 == visible_children.len() && !collapse;
        let branch = if is_last { "└─ " } else { "├─ " };
        let connector = if is_last { "   " } else { "│  " };
        let label = if child.is_dir {
            format!("{name}/")
        } else if let Some(size) = child.size {
            format!("{name}  {}", format_size(size))
        } else {
            name.to_string()
        };
        lines.push(PreviewLine::plain(format!("{prefix}{branch}{label}")));

        if child.children.is_empty() {
            continue;
        }
        if depth + 1 >= MAX_DEPTH {
            *depth_truncated = true;
            continue;
        }
        let next_prefix = format!("{prefix}{connector}");
        render_tree_children(
            child,
            &next_prefix,
            depth + 1,
            max_lines,
            lines,
            depth_truncated,
            breadth_collapsed,
        );
    }

    if collapse && lines.len() < max_lines + 1 {
        let hidden = node.children.len().saturating_sub(PER_DIR_SHOW_LIMIT);
        lines.push(PreviewLine::plain(format!("{prefix}└─ … {hidden} more")));
    }
}

fn format_size(size: u64) -> String {
    if size < 1024 {
        format!("{size}B")
    } else if size < 1024 * 1024 {
        format!("{:.1}KB", size as f64 / 1024.0)
    } else if size < 1024 * 1024 * 1024 {
        format!("{:.1}MB", size as f64 / 1024.0 / 1024.0)
    } else {
        format!("{:.1}GB", size as f64 / 1024.0 / 1024.0 / 1024.0)
    }
}

fn title_for(path: &Path) -> String {
    path.file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_preview_uses_branch_connectors() {
        let entries = vec![
            ArchiveEntry {
                path: "src/main.rs".to_string(),
                size: Some(1200),
                is_dir: false,
            },
            ArchiveEntry {
                path: "src/lib.rs".to_string(),
                size: Some(800),
                is_dir: false,
            },
            ArchiveEntry {
                path: "README.md".to_string(),
                size: Some(64),
                is_dir: false,
            },
        ];

        let (lines, depth_truncated, breadth_collapsed) = build_tree_lines(&entries, 40);
        let text = lines
            .into_iter()
            .map(|line| {
                line.spans
                    .into_iter()
                    .map(|span| span.text)
                    .collect::<String>()
            })
            .collect::<Vec<_>>();

        assert!(!depth_truncated);
        assert!(!breadth_collapsed);
        assert!(
            text.iter()
                .any(|line| line.contains("├─ src/") || line.contains("└─ src/"))
        );
        assert!(text.iter().any(|line| {
            line.contains("├─ lib.rs") || line.contains("└─ lib.rs") || line.contains("   lib.rs")
        }));
        assert!(text.iter().any(|line| line.contains("README.md")));
    }

    #[test]
    fn tree_preview_collapses_noisy_directory_levels() {
        let mut entries = Vec::new();
        for index in 0..60 {
            entries.push(ArchiveEntry {
                path: format!("node_modules/pkg-{index}/index.js"),
                size: Some(64),
                is_dir: false,
            });
        }

        let (lines, _depth_truncated, breadth_collapsed) = build_tree_lines(&entries, 200);
        let text = lines
            .into_iter()
            .map(|line| {
                line.spans
                    .into_iter()
                    .map(|span| span.text)
                    .collect::<String>()
            })
            .collect::<Vec<_>>();

        assert!(breadth_collapsed);
        assert!(text.iter().any(|line| line.contains("… 50 more")));
    }
}
