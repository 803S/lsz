pub mod archive;
pub mod external;
pub mod image;
pub mod markdown;
pub mod pdf;
pub mod text;

use crate::domain::{
    entry::{EntryItem, EntryKind},
    preview::{PreviewKind, PreviewLine, PreviewModel, PreviewResult, PreviewWrapMode},
};
use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::infra::analyzer::{self, Md5State};

#[derive(Debug, Clone, Copy)]
pub enum PreviewSurface {
    Inspect,
    Tui,
}

#[derive(Debug, Clone)]
pub struct PreviewRequest {
    pub path: PathBuf,
    pub kind: EntryKind,
    pub size: Option<u64>,
    pub viewport_hint: usize,
    pub surface: PreviewSurface,
}

pub trait PreviewProvider {
    fn supports(&self, request: &PreviewRequest) -> bool;
    fn load(&self, request: &PreviewRequest) -> Result<PreviewResult>;
}

pub struct PreviewService {
    providers: Vec<Box<dyn PreviewProvider + Send + Sync>>,
}

impl Default for PreviewService {
    fn default() -> Self {
        Self {
            providers: vec![
                Box::new(markdown::MarkdownProvider),
                Box::new(text::TextProvider),
                Box::new(archive::ArchiveProvider),
                Box::new(pdf::PdfProvider),
                Box::new(image::ImageProvider),
            ],
        }
    }
}

impl PreviewService {
    pub fn preview_entry(
        &self,
        entry: &EntryItem,
        viewport_hint: usize,
        surface: PreviewSurface,
    ) -> PreviewModel {
        let request = PreviewRequest {
            path: entry.path.clone(),
            kind: entry.kind.clone(),
            size: entry.size,
            viewport_hint,
            surface,
        };
        self.preview_request(&request)
    }

    pub fn preview_path(
        &self,
        path: &Path,
        viewport_hint: usize,
        surface: PreviewSurface,
    ) -> PreviewModel {
        let path = path.to_path_buf();
        let kind = if let Ok(meta) = std::fs::metadata(&path) {
            crate::infra::fs::detect_kind(&path, &meta)
        } else {
            EntryKind::Unknown
        };
        let size = std::fs::metadata(&path).ok().map(|meta| meta.len());
        let request = PreviewRequest {
            path,
            kind,
            size,
            viewport_hint,
            surface,
        };
        self.preview_request(&request)
    }

    fn preview_request(&self, request: &PreviewRequest) -> PreviewModel {
        if matches!(request.kind, EntryKind::Directory) {
            return directory_preview(&request.path, request.viewport_hint);
        }
        for provider in &self.providers {
            if provider.supports(request) {
                return provider
                    .load(request)
                    .map(|result| enrich_preview_result(request, result).into_model())
                    .unwrap_or_else(|err| PreviewModel::Failed {
                        path: request.path.clone(),
                        message: err.to_string(),
                    });
            }
        }
        enrich_preview_result(
            request,
            unsupported_preview(&request.path, "没有可用的预览器"),
        )
        .into_model()
    }
}

fn directory_preview(path: &Path, viewport_hint: usize) -> PreviewModel {
    directory_preview_result(path, viewport_hint).into_model()
}

fn directory_preview_result(path: &Path, viewport_hint: usize) -> PreviewResult {
    let mut lines = vec![
        PreviewLine::plain(path.display().to_string()),
        PreviewLine::plain(""),
    ];
    let mut child_count = 0usize;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten().take(viewport_hint.max(20)) {
            child_count += 1;
            let name = entry.file_name().to_string_lossy().to_string();
            lines.push(PreviewLine::plain(name));
        }
    }
    PreviewResult {
        kind: PreviewKind::Directory,
        title: path
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string()),
        subtitle: Some(path.display().to_string()),
        provider_name: "目录".to_string(),
        degraded: false,
        metadata: vec![("已显示子项".to_string(), child_count.to_string())],
        warnings: Vec::new(),
        actions: vec!["回车：进入/打开".to_string()],
        wrap_mode: PreviewWrapMode::NoWrap,
        lines,
        first_line_number: 1,
    }
}

pub fn unsupported_preview(path: &Path, message: &str) -> PreviewResult {
    PreviewResult {
        kind: PreviewKind::Unsupported,
        title: path
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string()),
        subtitle: Some(path.display().to_string()),
        provider_name: "未支持".to_string(),
        degraded: true,
        metadata: Vec::new(),
        warnings: vec![message.to_string()],
        actions: vec!["o：外部打开".to_string()],
        wrap_mode: PreviewWrapMode::Wrap,
        lines: vec![PreviewLine::plain(message)],
        first_line_number: 1,
    }
}

fn enrich_preview_result(request: &PreviewRequest, mut result: PreviewResult) -> PreviewResult {
    if matches!(request.kind, EntryKind::Directory) {
        return result;
    }
    let Ok(analysis) = analyzer::analyze_file(&request.path) else {
        return result;
    };

    append_unique_metadata(&mut result.metadata, "Real", analysis.file_signature);
    append_unique_metadata(&mut result.metadata, "MIME", analysis.mime_type);
    append_unique_metadata(
        &mut result.metadata,
        "Ext",
        if analysis.extension.is_empty() {
            "-".to_string()
        } else {
            format!(".{}", analysis.extension)
        },
    );
    append_unique_metadata(
        &mut result.metadata,
        "MD5",
        match analysis.md5_state {
            Md5State::Ready => analysis
                .md5_hash
                .unwrap_or_else(|| "<unavailable>".to_string()),
            Md5State::LargeFile => "<large file>".to_string(),
            Md5State::Unavailable => "<unavailable>".to_string(),
        },
    );
    if analysis.is_suspicious {
        let warning = "文件扩展名与真实类型不一致，可能存在伪装或可执行风险".to_string();
        if !result.warnings.iter().any(|line| line == &warning) {
            result.warnings.push(warning);
        }
    }
    result
}

fn append_unique_metadata(metadata: &mut Vec<(String, String)>, key: &str, value: String) {
    if metadata
        .iter()
        .any(|(existing_key, _)| existing_key.eq_ignore_ascii_case(key))
    {
        return;
    }
    metadata.push((key.to_string(), value));
}
