use crate::{
    domain::{
        entry::EntryKind,
        preview::{
            PreviewColor, PreviewKind, PreviewLine, PreviewResult, PreviewSpan, PreviewWrapMode,
        },
    },
    infra::{fs, syntax},
};
use anyhow::Result;
use std::{fs as stdfs, path::Path};

use super::{PreviewProvider, PreviewRequest};

pub struct MarkdownProvider;

impl PreviewProvider for MarkdownProvider {
    fn supports(&self, request: &PreviewRequest) -> bool {
        matches!(request.kind, EntryKind::Markdown)
    }

    fn load(&self, request: &PreviewRequest) -> Result<PreviewResult> {
        let content = stdfs::read_to_string(&request.path)?;
        let lines = render_markdown(&request.path, &content, request.viewport_hint.max(60));
        Ok(PreviewResult {
            kind: PreviewKind::Markdown,
            title: title_for(&request.path),
            subtitle: Some(request.path.display().to_string()),
            provider_name: "Markdown".to_string(),
            degraded: false,
            metadata: vec![("大小".to_string(), fs::size_label(request.size))],
            warnings: Vec::new(),
            actions: vec!["回车：阅读".to_string(), "o：外部打开".to_string()],
            wrap_mode: PreviewWrapMode::Wrap,
            lines,
            first_line_number: 1,
        })
    }
}

fn render_markdown(path: &Path, content: &str, max_lines: usize) -> Vec<PreviewLine> {
    let mut lines = Vec::new();
    let mut code_lang: Option<String> = None;
    let mut code_buffer = String::new();

    for raw_line in content.lines() {
        if lines.len() >= max_lines {
            break;
        }
        let line = raw_line.trim_end_matches(['\r', '\n']);
        if let Some(lang) = parse_fence(line) {
            if code_lang.is_some() {
                flush_code_block(
                    path,
                    code_lang.as_deref(),
                    &code_buffer,
                    max_lines,
                    &mut lines,
                );
                code_buffer.clear();
                code_lang = None;
            } else {
                code_lang = Some(lang.unwrap_or_default());
            }
            continue;
        }
        if code_lang.is_some() {
            code_buffer.push_str(line);
            code_buffer.push('\n');
            continue;
        }
        lines.push(render_markdown_line(line));
    }
    if code_lang.is_some() && lines.len() < max_lines {
        flush_code_block(
            path,
            code_lang.as_deref(),
            &code_buffer,
            max_lines,
            &mut lines,
        );
    }
    if lines.is_empty() {
        lines.push(PreviewLine::plain("（空 Markdown）"));
    }
    lines
}

fn parse_fence(line: &str) -> Option<Option<String>> {
    let trimmed = line.trim();
    if !trimmed.starts_with("```") {
        return None;
    }
    let lang = trimmed.trim_start_matches("```").trim();
    if lang.is_empty() {
        Some(None)
    } else {
        Some(Some(lang.to_string()))
    }
}

fn flush_code_block(
    path: &Path,
    syntax_hint: Option<&str>,
    content: &str,
    max_lines: usize,
    lines: &mut Vec<PreviewLine>,
) {
    let remaining = max_lines.saturating_sub(lines.len());
    if remaining == 0 {
        return;
    }
    let highlighted = syntax::highlight_code_with_hint(path, syntax_hint, content, remaining);
    if highlighted.is_empty() {
        lines.push(PreviewLine::plain(""));
    } else {
        lines.extend(highlighted);
    }
}

fn render_markdown_line(line: &str) -> PreviewLine {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return PreviewLine::plain("");
    }
    if trimmed.chars().all(|ch| ch == '-' || ch == '=') && trimmed.len() >= 3 {
        return PreviewLine {
            spans: vec![PreviewSpan {
                text: "─".repeat(trimmed.len().min(32)),
                fg: Some(color(120, 120, 120)),
                bold: false,
                dim: true,
            }],
        };
    }
    if let Some(rest) = trimmed.strip_prefix("# ") {
        return inline_styled_line(rest, Some(color(0, 180, 255)), true, false);
    }
    if let Some(rest) = trimmed.strip_prefix("## ") {
        return inline_styled_line(rest, Some(color(0, 200, 160)), true, false);
    }
    if let Some(rest) = trimmed.strip_prefix("### ") {
        return inline_styled_line(rest, Some(color(255, 180, 0)), true, false);
    }
    if let Some(rest) = trimmed.strip_prefix("> ") {
        let mut spans = vec![PreviewSpan {
            text: "▎ ".to_string(),
            fg: Some(color(120, 180, 200)),
            bold: true,
            dim: false,
        }];
        spans.extend(inline_spans(rest, Some(color(185, 205, 220)), false, false));
        return PreviewLine { spans };
    }
    if let Some(rest) = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("+ "))
    {
        let mut spans = vec![PreviewSpan {
            text: "• ".to_string(),
            fg: Some(color(255, 180, 0)),
            bold: true,
            dim: false,
        }];
        spans.extend(inline_spans(rest, None, false, false));
        return PreviewLine { spans };
    }
    if let Some(marker_len) = ordered_list_marker_len(trimmed) {
        let marker = trimmed[..marker_len].trim().to_string();
        let rest = trimmed[marker_len..].trim_start();
        let mut spans = vec![PreviewSpan {
            text: format!("{marker} "),
            fg: Some(color(255, 180, 0)),
            bold: true,
            dim: false,
        }];
        spans.extend(inline_spans(rest, None, false, false));
        return PreviewLine { spans };
    }
    inline_styled_line(line, None, false, false)
}

fn ordered_list_marker_len(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
    }
    if index == 0 || index + 1 >= bytes.len() {
        return None;
    }
    if matches!(bytes[index], b'.' | b')') && bytes[index + 1] == b' ' {
        Some(index + 1)
    } else {
        None
    }
}

fn inline_styled_line(text: &str, fg: Option<PreviewColor>, bold: bool, dim: bool) -> PreviewLine {
    PreviewLine {
        spans: inline_spans(text, fg, bold, dim),
    }
}

fn inline_spans(text: &str, fg: Option<PreviewColor>, bold: bool, dim: bool) -> Vec<PreviewSpan> {
    let mut spans = Vec::new();
    let mut remaining = text;
    let mut code = false;
    loop {
        let Some(index) = remaining.find('`') else {
            if !remaining.is_empty() {
                spans.push(PreviewSpan {
                    text: remaining.to_string(),
                    fg: if code { Some(color(255, 214, 102)) } else { fg },
                    bold: if code { true } else { bold },
                    dim,
                });
            }
            break;
        };
        let (head, tail) = remaining.split_at(index);
        if !head.is_empty() {
            spans.push(PreviewSpan {
                text: head.to_string(),
                fg: if code { Some(color(255, 214, 102)) } else { fg },
                bold: if code { true } else { bold },
                dim,
            });
        }
        remaining = &tail[1..];
        code = !code;
    }
    if spans.is_empty() {
        spans.push(PreviewSpan {
            text: String::new(),
            fg,
            bold,
            dim,
        });
    }
    spans
}

fn color(r: u8, g: u8, b: u8) -> PreviewColor {
    PreviewColor { r, g, b }
}

fn title_for(path: &Path) -> String {
    path.file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::{
            entry::EntryKind,
            preview::{PreviewKind, PreviewWrapMode},
        },
        infra::providers::{PreviewProvider, PreviewSurface},
    };
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn unique_path(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("lsz-markdown-test-{name}-{nanos}.md"))
    }

    #[test]
    fn markdown_provider_renders_headings_lists_and_code_blocks() {
        let path = unique_path("README");
        fs::write(
            &path,
            "# 标题\n\n- 列表项\n\n> 引用段落\n\n```rust\nfn main() {}\n```\n",
        )
        .expect("write markdown");

        let preview = MarkdownProvider
            .load(&PreviewRequest {
                path: path.clone(),
                kind: EntryKind::Markdown,
                size: fs::metadata(&path).ok().map(|meta| meta.len()),
                viewport_hint: 80,
                surface: PreviewSurface::Tui,
            })
            .expect("load preview");

        assert_eq!(preview.kind, PreviewKind::Markdown);
        assert_eq!(preview.wrap_mode, PreviewWrapMode::Wrap);
        assert_eq!(preview.lines[0].spans[0].text, "标题");
        assert!(
            preview
                .lines
                .iter()
                .any(|line| line.spans.first().map(|span| span.text.as_str()) == Some("• "))
        );
        assert!(preview.lines.iter().any(|line| {
            line.spans
                .iter()
                .map(|span| span.text.as_str())
                .collect::<String>()
                .contains("fn main()")
        }));
        assert!(!preview.lines.iter().any(|line| {
            line.spans
                .iter()
                .any(|span| span.text.contains("```") || span.text.contains("# "))
        }));

        fs::remove_file(path).expect("cleanup markdown");
    }
}
