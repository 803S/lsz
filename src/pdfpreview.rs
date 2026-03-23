use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct PdfInfo {
    pub title: String,
    pub author: String,
    pub page_count: usize,
    pub file_size: u64,
}

pub fn get_pdf_info<P: AsRef<Path>>(path: P) -> Option<PdfInfo> {
    let path = path.as_ref();
    let metadata = fs::metadata(path).ok()?;
    let bytes = fs::read(path).ok()?;
    let content = String::from_utf8_lossy(&bytes);

    let title = find_pdf_value(&content, "/Title").unwrap_or_default();
    let author = find_pdf_value(&content, "/Author").unwrap_or_default();
    let page_count = content
        .matches("/Type /Page")
        .count()
        .max(content.matches("/Count ").count());

    Some(PdfInfo {
        title,
        author,
        page_count,
        file_size: metadata.len(),
    })
}

fn find_pdf_value(content: &str, key: &str) -> Option<String> {
    let idx = content.find(key)?;
    let tail = &content[idx + key.len()..];
    let start = tail.find('(')? + 1;
    let tail = &tail[start..];
    let end = tail.find(')')?;
    Some(tail[..end].trim().to_string())
}

pub fn is_pdf_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
}

pub fn extract_pdf_text<P: AsRef<Path>>(path: P, max_pages: usize) -> Option<String> {
    let path = path.as_ref();
    if let Ok(output) = Command::new("pdftotext")
        .args([
            "-layout",
            "-l",
            &max_pages.to_string(),
            path.to_str().unwrap_or(""),
            "-",
        ])
        .output()
    {
        if output.status.success() {
            return Some(String::from_utf8_lossy(&output.stdout).to_string());
        }
    }
    let content = fs::read(path).ok()?;
    let raw = String::from_utf8_lossy(&content);
    let text: String = raw
        .chars()
        .filter(|c| c.is_ascii_graphic() || c.is_ascii_whitespace())
        .collect();
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        None
    } else {
        Some(compact.chars().take(8000).collect())
    }
}

pub fn open_with_default_app<P: AsRef<Path>>(path: P) -> std::io::Result<()> {
    let path = path.as_ref();
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open").arg(path).spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(path).spawn()?;
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", &path.to_string_lossy()])
            .spawn()?;
    }
    Ok(())
}

pub fn is_media_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase()
            .as_str(),
        "mp4"
            | "mkv"
            | "avi"
            | "mov"
            | "wmv"
            | "flv"
            | "webm"
            | "mp3"
            | "wav"
            | "flac"
            | "aac"
            | "ogg"
            | "m4a"
            | "pdf"
            | "doc"
            | "docx"
            | "xls"
            | "xlsx"
            | "ppt"
            | "pptx"
    )
}
