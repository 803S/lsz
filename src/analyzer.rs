use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::Command;

const HASH_LIMIT: u64 = 5 * 1024 * 1024;

#[derive(Debug, Clone, Default)]
pub struct FileSecurityInfo {
    pub mime_type: String,
    pub extension: String,
    pub is_suspicious: bool,
    pub md5_hash: Option<String>,
    pub file_signature: String,
}

fn sniff_signature(buf: &[u8]) -> (&'static str, &'static str) {
    if buf.starts_with(b"\x7fELF") {
        ("application/x-elf", "ELF Executable")
    } else if buf.starts_with(b"MZ") {
        ("application/x-msdownload", "Windows PE")
    } else if buf.starts_with(&[0xFE, 0xED, 0xFA]) || buf.starts_with(&[0xCF, 0xFA, 0xED, 0xFE]) {
        ("application/x-mach-binary", "Mach-O Binary")
    } else if buf.starts_with(b"PK\x03\x04") {
        ("application/zip", "ZIP Archive")
    } else if buf.starts_with(b"\x1F\x8B") {
        ("application/gzip", "GZIP Compressed")
    } else if buf.starts_with(b"%PDF-") {
        ("application/pdf", "PDF Document")
    } else if buf.starts_with(b"\x89PNG\r\n\x1a\n") {
        ("image/png", "PNG Image")
    } else if buf.starts_with(&[0xFF, 0xD8, 0xFF]) {
        ("image/jpeg", "JPEG Image")
    } else if buf.starts_with(b"GIF87a") || buf.starts_with(b"GIF89a") {
        ("image/gif", "GIF Image")
    } else if buf.starts_with(b"RIFF") && buf.get(8..12) == Some(b"WEBP") {
        ("image/webp", "WebP Image")
    } else {
        ("application/octet-stream", "Binary Data")
    }
}

fn compute_md5(path: &Path) -> Option<String> {
    let output = Command::new("md5sum").arg(path).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let line = String::from_utf8_lossy(&output.stdout);
    line.split_whitespace().next().map(|s| s.to_string())
}

pub fn analyze_file<P: AsRef<Path>>(path: P) -> std::io::Result<FileSecurityInfo> {
    let path = path.as_ref();
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let mut file = File::open(path)?;
    let metadata = file.metadata()?;
    let mut buffer = [0u8; 8192];
    let read = file.read(&mut buffer)?;
    let (mime_type, file_signature) = sniff_signature(&buffer[..read]);

    let safe_exts = [
        "jpg", "jpeg", "png", "gif", "bmp", "webp", "svg", "ico", "txt", "md", "pdf", "doc",
        "docx", "xls", "xlsx", "mp3", "mp4", "avi", "mkv", "mov", "wav", "flac", "zip", "rar",
        "7z", "tar", "gz", "bz2",
    ];
    let is_executable_mime = matches!(
        mime_type,
        "application/x-elf" | "application/x-mach-binary" | "application/x-msdownload"
    );

    Ok(FileSecurityInfo {
        mime_type: mime_type.to_string(),
        extension,
        is_suspicious: safe_exts.contains(
            &path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase()
                .as_str(),
        ) && is_executable_mime,
        md5_hash: if metadata.is_file() && metadata.len() <= HASH_LIMIT {
            compute_md5(path)
        } else {
            None
        },
        file_signature: file_signature.to_string(),
    })
}
