use crate::{
    domain::{
        entry::EntryKind,
        preview::{PreviewKind, PreviewLine, PreviewResult, PreviewWrapMode},
    },
    infra::{fs, providers::PreviewSurface, syntax},
};
use anyhow::Result;
use std::{
    fs as stdfs,
    io::{BufRead, BufReader},
    path::Path,
};

use super::{PreviewProvider, PreviewRequest};

pub struct TextProvider;

impl PreviewProvider for TextProvider {
    fn supports(&self, request: &PreviewRequest) -> bool {
        matches!(
            request.kind,
            EntryKind::Text | EntryKind::Code | EntryKind::Binary | EntryKind::Unknown
        )
    }

    fn load(&self, request: &PreviewRequest) -> Result<PreviewResult> {
        let path = &request.path;
        let size = request.size.unwrap_or(0);
        let max_lines = request.viewport_hint.max(match request.surface {
            PreviewSurface::Inspect => 140,
            PreviewSurface::Tui => 220,
        });
        let is_mid_sized = (256 * 1024..=2 * 1024 * 1024).contains(&size);
        let is_large = size > 2 * 1024 * 1024;
        let degraded = is_mid_sized || is_large;
        let mut lines = Vec::new();
        let mut warnings = Vec::new();

        match request.kind {
            EntryKind::Code if !is_large => {
                if is_mid_sized {
                    match read_excerpt(path, max_lines) {
                        Some(content) => {
                            lines = syntax::highlight_code(path, &content, max_lines);
                            warnings.push(format!(
                                "中等文件：当前只高亮前 {max_lines} 行，继续滚动时建议按 n 关闭行号后复制"
                            ));
                        }
                        None => lines.push(PreviewLine::plain("二进制或不可读内容")),
                    }
                } else {
                    match stdfs::read_to_string(path) {
                        Ok(content) => {
                            let total_lines = content.lines().count();
                            lines = syntax::highlight_code(path, &content, total_lines.max(1));
                        }
                        Err(_) => lines.push(PreviewLine::plain("二进制或不可读内容")),
                    }
                }
            }
            _ => {
                if is_large {
                    warnings.push("大文件：已关闭语法高亮，仅显示前部文本".to_string());
                }
                if let Ok(file) = stdfs::File::open(path) {
                    for line in BufReader::new(file)
                        .lines()
                        .map_while(Result::ok)
                        .take(max_lines)
                    {
                        lines.push(PreviewLine::plain(line));
                    }
                }
                if lines.is_empty() {
                    lines.push(PreviewLine::plain("二进制或不可读内容"));
                }
            }
        }

        let preview_kind = match request.kind {
            EntryKind::Code => PreviewKind::Code,
            EntryKind::Text | EntryKind::Unknown => PreviewKind::Text,
            _ => PreviewKind::Binary,
        };

        if is_mid_sized && warnings.is_empty() {
            warnings.push(format!("中等文件：仅加载前 {max_lines} 行窗口内容"));
        }

        Ok(PreviewResult {
            kind: preview_kind,
            title: title_for(path),
            subtitle: Some(path.display().to_string()),
            provider_name: "文本".to_string(),
            degraded,
            metadata: vec![
                ("大小".to_string(), fs::size_label(request.size)),
                (
                    "高亮".to_string(),
                    if is_large {
                        "关闭".to_string()
                    } else if is_mid_sized {
                        "窗口级".to_string()
                    } else {
                        "开启".to_string()
                    },
                ),
            ],
            warnings,
            actions: vec!["回车：阅读".to_string(), "o：外部打开".to_string()],
            wrap_mode: if matches!(preview_kind, PreviewKind::Code) {
                PreviewWrapMode::NoWrap
            } else {
                PreviewWrapMode::Wrap
            },
            lines,
            first_line_number: 1,
        })
    }
}

fn read_excerpt(path: &Path, max_lines: usize) -> Option<String> {
    let file = stdfs::File::open(path).ok()?;
    let mut excerpt = String::new();
    for line in BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .take(max_lines)
    {
        excerpt.push_str(&line);
        excerpt.push('\n');
    }
    if excerpt.is_empty() {
        None
    } else {
        Some(excerpt)
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
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn unique_path(name: &str, ext: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("lsz-provider-test-{name}-{nanos}.{ext}"))
    }

    #[test]
    fn small_code_file_keeps_highlight() {
        let path = unique_path("small", "rs");
        fs::write(&path, "fn main() {\n    println!(\"hi\");\n}\n").expect("write");
        let provider = TextProvider;
        let preview = provider
            .load(&PreviewRequest {
                path: path.clone(),
                kind: EntryKind::Code,
                size: fs::metadata(&path).ok().map(|meta| meta.len()),
                viewport_hint: 20,
                surface: PreviewSurface::Tui,
            })
            .expect("load preview");

        assert!(!preview.degraded);
        assert_eq!(preview.kind, PreviewKind::Code);
        assert!(preview.lines.len() >= 3);

        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn large_code_file_disables_highlight() {
        let path = unique_path("large", "rs");
        let content = "let value = 1;\n".repeat(170_000);
        fs::write(&path, content).expect("write");
        let provider = TextProvider;
        let preview = provider
            .load(&PreviewRequest {
                path: path.clone(),
                kind: EntryKind::Code,
                size: fs::metadata(&path).ok().map(|meta| meta.len()),
                viewport_hint: 20,
                surface: PreviewSurface::Tui,
            })
            .expect("load preview");

        assert!(preview.degraded);
        assert!(preview.warnings.iter().any(|line| line.contains("大文件")));
        assert!(
            preview
                .metadata
                .iter()
                .any(|(key, value)| key == "高亮" && value == "关闭")
        );

        fs::remove_file(path).expect("cleanup");
    }
}
