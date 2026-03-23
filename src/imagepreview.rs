use crossterm::event::DisableMouseCapture;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub enum ImageBackend {
    Kitty,
    Ueberzug,
    Chafa,
    External,
}

#[derive(Debug, Clone)]
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub color_type: String,
    pub exif_data: Vec<(String, String)>,
    pub backend: ImageBackend,
}

fn has_cmd(cmd: &str) -> bool {
    Command::new("sh")
        .arg("-lc")
        .arg(format!("command -v {} >/dev/null 2>&1", cmd))
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn supports_kitty_graphics() -> bool {
    std::env::var("TERM")
        .map(|t| t.contains("kitty"))
        .unwrap_or(false)
        || std::env::var("TERM_PROGRAM")
            .map(|t| matches!(t.as_str(), "WezTerm" | "iTerm.app"))
            .unwrap_or(false)
}

fn detect_backend() -> ImageBackend {
    if supports_kitty_graphics() {
        ImageBackend::Kitty
    } else if has_cmd("ueberzugpp") {
        ImageBackend::Ueberzug
    } else if has_cmd("chafa") {
        ImageBackend::Chafa
    } else {
        ImageBackend::External
    }
}

fn parse_png(buf: &[u8]) -> Option<(u32, u32)> {
    if buf.len() >= 24 && &buf[..8] == b"\x89PNG\r\n\x1a\n" {
        let w = u32::from_be_bytes([buf[16], buf[17], buf[18], buf[19]]);
        let h = u32::from_be_bytes([buf[20], buf[21], buf[22], buf[23]]);
        Some((w, h))
    } else {
        None
    }
}

fn parse_gif(buf: &[u8]) -> Option<(u32, u32)> {
    if buf.len() >= 10 && (&buf[..6] == b"GIF87a" || &buf[..6] == b"GIF89a") {
        let w = u16::from_le_bytes([buf[6], buf[7]]) as u32;
        let h = u16::from_le_bytes([buf[8], buf[9]]) as u32;
        Some((w, h))
    } else {
        None
    }
}

fn parse_bmp(buf: &[u8]) -> Option<(u32, u32)> {
    if buf.len() >= 26 && &buf[..2] == b"BM" {
        let w = u32::from_le_bytes([buf[18], buf[19], buf[20], buf[21]]);
        let h = u32::from_le_bytes([buf[22], buf[23], buf[24], buf[25]]);
        Some((w, h))
    } else {
        None
    }
}

fn parse_jpeg(buf: &[u8]) -> Option<(u32, u32)> {
    if !buf.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let mut i = 2;
    while i + 9 < buf.len() {
        if buf[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = buf[i + 1];
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
            let h = u16::from_be_bytes([buf[i + 5], buf[i + 6]]) as u32;
            let w = u16::from_be_bytes([buf[i + 7], buf[i + 8]]) as u32;
            return Some((w, h));
        }
        if i + 4 >= buf.len() {
            break;
        }
        let len = u16::from_be_bytes([buf[i + 2], buf[i + 3]]) as usize;
        if len < 2 {
            break;
        }
        i += 2 + len;
    }
    None
}

pub fn is_image_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase()
            .as_str(),
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "svg" | "ico" | "tiff" | "tif"
    )
}

pub fn get_image_info<P: AsRef<Path>>(path: P) -> Option<ImageInfo> {
    let path = path.as_ref();
    let mut file = fs::File::open(path).ok()?;
    let mut buf = vec![0u8; 65536];
    let read = file.read(&mut buf).ok()?;
    buf.truncate(read);
    let dims = parse_png(&buf)
        .or_else(|| parse_gif(&buf))
        .or_else(|| parse_bmp(&buf))
        .or_else(|| parse_jpeg(&buf));
    let format = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("unknown")
        .to_uppercase();
    let color_type = "Preview via terminal adapter / external app".to_string();
    let (width, height) = dims.unwrap_or((0, 0));
    Some(ImageInfo {
        width,
        height,
        format,
        color_type,
        exif_data: Vec::new(),
        backend: detect_backend(),
    })
}

pub fn backend_name(backend: &ImageBackend) -> &'static str {
    match backend {
        ImageBackend::Kitty => "Kitty / Inline protocol",
        ImageBackend::Ueberzug => "Überzug++",
        ImageBackend::Chafa => "Chafa",
        ImageBackend::External => "System viewer",
    }
}

pub fn run_image_fullscreen<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = path.as_ref();
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;

    match detect_backend() {
        ImageBackend::Kitty => {
            let _ = super::pdfpreview::open_with_default_app(path);
        }
        ImageBackend::Ueberzug => {
            let _ = Command::new("ueberzugpp").arg("layer").status();
        }
        ImageBackend::Chafa => {
            let _ = Command::new("chafa").arg(path).status();
        }
        ImageBackend::External => {
            let _ = super::pdfpreview::open_with_default_app(path);
        }
    }

    println!(
        "已使用 {:?} 尝试打开图片，按 Enter 返回...",
        detect_backend()
    );
    let mut s = String::new();
    let _ = io::stdin().read_line(&mut s);
    Ok(())
}
