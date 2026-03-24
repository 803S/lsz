use crate::domain::{
    entry::EntryKind,
    preview::{PreviewKind, PreviewLine, PreviewResult, PreviewWrapMode},
};
use anyhow::Result;
use std::{fs, path::Path, process::Command};

use super::{PreviewProvider, PreviewRequest};

pub struct PdfProvider;

impl PreviewProvider for PdfProvider {
    fn supports(&self, request: &PreviewRequest) -> bool {
        matches!(request.kind, EntryKind::Pdf)
    }

    fn load(&self, request: &PreviewRequest) -> Result<PreviewResult> {
        let path = &request.path;
        let bytes = fs::read(path)?;
        let raw = String::from_utf8_lossy(&bytes);
        let page_count = raw
            .matches("/Type /Page")
            .count()
            .max(raw.matches("/Count ").count());
        let title = find_pdf_value(&raw, "/Title").unwrap_or_default();
        let author = find_pdf_value(&raw, "/Author").unwrap_or_default();
        let mut lines = Vec::new();
        let mut degraded = false;
        let mut warnings = Vec::new();

        if let Ok(output) = Command::new("pdftotext")
            .args([
                "-layout",
                "-l",
                &request.viewport_hint.max(5).to_string(),
                &path.to_string_lossy(),
                "-",
            ])
            .output()
        {
            if output.status.success() {
                for line in String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .take(request.viewport_hint.max(40))
                {
                    lines.push(PreviewLine::plain(line));
                }
            }
        }

        if lines.is_empty() {
            degraded = true;
            warnings.push("当前环境没有可靠的 PDF 文本提取器".to_string());
            lines.push(PreviewLine::plain("PDF 文本预览不可用"));
        }

        let mut metadata = vec![("页数".to_string(), page_count.to_string())];
        if !title.is_empty() {
            metadata.push(("标题".to_string(), title));
        }
        if !author.is_empty() {
            metadata.push(("作者".to_string(), author));
        }

        Ok(PreviewResult {
            kind: PreviewKind::Pdf,
            title: title_for(path),
            subtitle: Some(path.display().to_string()),
            provider_name: "PDF".to_string(),
            degraded,
            metadata,
            warnings,
            actions: vec!["o：外部打开".to_string()],
            wrap_mode: PreviewWrapMode::NoWrap,
            lines,
            first_line_number: 1,
        })
    }
}

fn find_pdf_value(content: &str, key: &str) -> Option<String> {
    let index = content.find(key)?;
    let tail = &content[index + key.len()..];
    let start = tail.find('(')? + 1;
    let tail = &tail[start..];
    let end = tail.find(')')?;
    Some(tail[..end].trim().to_string())
}

fn title_for(path: &Path) -> String {
    path.file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string())
}
