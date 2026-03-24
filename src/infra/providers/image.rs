use crate::domain::{
    entry::EntryKind,
    preview::{PreviewKind, PreviewLine, PreviewResult, PreviewWrapMode},
};
use anyhow::Result;
use std::{fs, io::Read, path::Path};

use super::{PreviewProvider, PreviewRequest};

pub struct ImageProvider;

impl PreviewProvider for ImageProvider {
    fn supports(&self, request: &PreviewRequest) -> bool {
        matches!(request.kind, EntryKind::Image)
    }

    fn load(&self, request: &PreviewRequest) -> Result<PreviewResult> {
        let path = &request.path;
        let mut file = fs::File::open(path)?;
        let mut buf = vec![0u8; 65536];
        let read = file.read(&mut buf)?;
        buf.truncate(read);
        let (width, height) = parse_png(&buf)
            .or_else(|| parse_gif(&buf))
            .or_else(|| parse_bmp(&buf))
            .or_else(|| parse_jpeg(&buf))
            .unwrap_or((0, 0));
        Ok(PreviewResult {
            kind: PreviewKind::Image,
            title: title_for(path),
            subtitle: Some(path.display().to_string()),
            provider_name: "图片".to_string(),
            degraded: false,
            metadata: vec![
                (
                    "格式".to_string(),
                    path.extension()
                        .and_then(|v| v.to_str())
                        .unwrap_or("-")
                        .to_uppercase(),
                ),
                ("尺寸".to_string(), format!("{width}x{height}")),
            ],
            warnings: Vec::new(),
            actions: vec!["o：外部打开".to_string()],
            wrap_mode: PreviewWrapMode::Wrap,
            lines: vec![
                PreviewLine::plain("图片元信息预览"),
                PreviewLine::plain(format!("尺寸: {width}x{height}")),
            ],
            first_line_number: 1,
        })
    }
}

fn parse_png(buf: &[u8]) -> Option<(u32, u32)> {
    if buf.len() >= 24 && &buf[..8] == b"\x89PNG\r\n\x1a\n" {
        Some((
            u32::from_be_bytes([buf[16], buf[17], buf[18], buf[19]]),
            u32::from_be_bytes([buf[20], buf[21], buf[22], buf[23]]),
        ))
    } else {
        None
    }
}

fn parse_gif(buf: &[u8]) -> Option<(u32, u32)> {
    if buf.len() >= 10 && (&buf[..6] == b"GIF87a" || &buf[..6] == b"GIF89a") {
        Some((
            u16::from_le_bytes([buf[6], buf[7]]) as u32,
            u16::from_le_bytes([buf[8], buf[9]]) as u32,
        ))
    } else {
        None
    }
}

fn parse_bmp(buf: &[u8]) -> Option<(u32, u32)> {
    if buf.len() >= 26 && &buf[..2] == b"BM" {
        Some((
            u32::from_le_bytes([buf[18], buf[19], buf[20], buf[21]]),
            u32::from_le_bytes([buf[22], buf[23], buf[24], buf[25]]),
        ))
    } else {
        None
    }
}

fn parse_jpeg(buf: &[u8]) -> Option<(u32, u32)> {
    if !buf.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let mut index = 2;
    while index + 9 < buf.len() {
        if buf[index] != 0xFF {
            index += 1;
            continue;
        }
        let marker = buf[index + 1];
        if matches!(
            marker,
            0xC0 | 0xC1
                | 0xC2
                | 0xC3
                | 0xC5
                | 0xC6
                | 0xC7
                | 0xC9
                | 0xCA
                | 0xCB
                | 0xCD
                | 0xCE
                | 0xCF
        ) {
            return Some((
                u16::from_be_bytes([buf[index + 7], buf[index + 8]]) as u32,
                u16::from_be_bytes([buf[index + 5], buf[index + 6]]) as u32,
            ));
        }
        if index + 4 >= buf.len() {
            break;
        }
        let len = u16::from_be_bytes([buf[index + 2], buf[index + 3]]) as usize;
        if len < 2 {
            break;
        }
        index += 2 + len;
    }
    None
}

fn title_for(path: &Path) -> String {
    path.file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string())
}
