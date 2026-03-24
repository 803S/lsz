use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
pub struct PreviewColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug, Clone, Default)]
pub struct PreviewSpan {
    pub text: String,
    pub fg: Option<PreviewColor>,
    pub bold: bool,
    pub dim: bool,
}

#[derive(Debug, Clone, Default)]
pub struct PreviewLine {
    pub spans: Vec<PreviewSpan>,
}

impl PreviewLine {
    pub fn plain(text: impl Into<String>) -> Self {
        let text = text.into().trim_end_matches(['\r', '\n']).to_string();
        Self {
            spans: vec![PreviewSpan {
                text,
                fg: None,
                bold: false,
                dim: false,
            }],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewKind {
    Directory,
    Markdown,
    Code,
    Text,
    Archive,
    Pdf,
    Image,
    Binary,
    Unsupported,
}

impl PreviewKind {
    pub fn label(self) -> &'static str {
        match self {
            PreviewKind::Directory => "目录",
            PreviewKind::Markdown => "Markdown",
            PreviewKind::Code => "代码",
            PreviewKind::Text => "文本",
            PreviewKind::Archive => "压缩包",
            PreviewKind::Pdf => "PDF",
            PreviewKind::Image => "图片",
            PreviewKind::Binary => "二进制",
            PreviewKind::Unsupported => "未支持",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewWrapMode {
    Wrap,
    NoWrap,
}

impl PreviewWrapMode {
    pub fn should_wrap(self) -> bool {
        matches!(self, Self::Wrap)
    }
}

#[derive(Debug, Clone)]
pub struct PreviewDocument {
    pub kind: PreviewKind,
    pub title: String,
    #[allow(dead_code)]
    pub subtitle: Option<String>,
    pub provider_name: String,
    pub degraded: bool,
    pub metadata: Vec<(String, String)>,
    pub warnings: Vec<String>,
    pub actions: Vec<String>,
    pub wrap_mode: PreviewWrapMode,
    pub lines: Vec<PreviewLine>,
    pub first_line_number: usize,
}

#[derive(Debug, Clone)]
pub struct PreviewResult {
    pub kind: PreviewKind,
    pub title: String,
    pub subtitle: Option<String>,
    pub provider_name: String,
    pub degraded: bool,
    pub metadata: Vec<(String, String)>,
    pub warnings: Vec<String>,
    pub actions: Vec<String>,
    pub wrap_mode: PreviewWrapMode,
    pub lines: Vec<PreviewLine>,
    pub first_line_number: usize,
}

impl PreviewResult {
    pub fn into_model(self) -> PreviewModel {
        PreviewModel::Document(PreviewDocument {
            kind: self.kind,
            title: self.title,
            subtitle: self.subtitle,
            provider_name: self.provider_name,
            degraded: self.degraded,
            metadata: self.metadata,
            warnings: self.warnings,
            actions: self.actions,
            wrap_mode: self.wrap_mode,
            lines: self.lines,
            first_line_number: self.first_line_number,
        })
    }
}

#[derive(Debug, Clone)]
pub enum PreviewModel {
    Loading { path: PathBuf },
    Document(PreviewDocument),
    Failed { path: PathBuf, message: String },
}
