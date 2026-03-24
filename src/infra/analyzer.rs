use std::{fs::File, io::Read, path::Path, process::Command};

const HASH_LIMIT: u64 = 5 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct FileAnalysis {
    pub mime_type: String,
    pub extension: String,
    pub is_suspicious: bool,
    pub md5_hash: Option<String>,
    pub md5_state: Md5State,
    pub file_signature: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Md5State {
    Ready,
    LargeFile,
    Unavailable,
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

fn detect_mime_with_file(path: &Path) -> Option<String> {
    let output = Command::new("file")
        .args(["--mime-type", "-Lb"])
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let mime = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!mime.is_empty()).then_some(mime)
}

fn compute_md5(path: &Path) -> Option<String> {
    let output = Command::new("md5sum").arg(path).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let line = String::from_utf8_lossy(&output.stdout);
    line.split_whitespace()
        .next()
        .map(|value| value.to_string())
}

fn describe_text_signature(path: &Path, mime_type: &str) -> String {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if name == "dockerfile" {
        "Dockerfile".to_string()
    } else if name == "makefile" {
        "Makefile".to_string()
    } else if name == "justfile" {
        "Justfile".to_string()
    } else if mime_type == "text/markdown" {
        "Markdown Document".to_string()
    } else if mime_type.starts_with("text/") {
        "Text / Source".to_string()
    } else if mime_type == "application/json" {
        "JSON Document".to_string()
    } else {
        "Structured Text".to_string()
    }
}

fn normalize_mime_by_extension(path: &Path, mime_type: String) -> String {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "md" | "markdown" => "text/markdown".to_string(),
        "json" => "application/json".to_string(),
        "yaml" | "yml" => "text/yaml".to_string(),
        "toml" => "text/x-toml".to_string(),
        "xml" => "application/xml".to_string(),
        _ => mime_type,
    }
}

pub fn analyze_file(path: &Path) -> std::io::Result<FileAnalysis> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let mut file = File::open(path)?;
    let metadata = file.metadata()?;
    let mut buffer = [0u8; 8192];
    let read = file.read(&mut buffer)?;
    let (sniffed_mime, sniffed_signature) = sniff_signature(&buffer[..read]);
    let mime_type = normalize_mime_by_extension(
        path,
        detect_mime_with_file(path).unwrap_or_else(|| sniffed_mime.to_string()),
    );
    let file_signature = if sniffed_signature == "Binary Data"
        && !mime_type.starts_with("application/octet-stream")
    {
        describe_text_signature(path, &mime_type)
    } else {
        sniffed_signature.to_string()
    };

    let safe_exts = [
        "jpg", "jpeg", "png", "gif", "bmp", "webp", "svg", "ico", "txt", "md", "pdf", "doc",
        "docx", "xls", "xlsx", "mp3", "mp4", "avi", "mkv", "mov", "wav", "flac", "zip", "rar",
        "7z", "tar", "gz", "bz2",
    ];
    let is_executable_mime = matches!(
        sniffed_mime,
        "application/x-elf" | "application/x-mach-binary" | "application/x-msdownload"
    ) || matches!(
        mime_type.as_str(),
        "application/x-elf" | "application/x-mach-binary" | "application/x-msdownload"
    );

    let (md5_hash, md5_state) = if metadata.is_file() && metadata.len() <= HASH_LIMIT {
        match compute_md5(path) {
            Some(hash) => (Some(hash), Md5State::Ready),
            None => (None, Md5State::Unavailable),
        }
    } else {
        (None, Md5State::LargeFile)
    };

    Ok(FileAnalysis {
        mime_type,
        is_suspicious: safe_exts.contains(&extension.as_str()) && is_executable_mime,
        extension,
        md5_hash,
        md5_state,
        file_signature,
    })
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
        std::env::temp_dir().join(format!("lsz-analyzer-test-{name}-{nanos}.{ext}"))
    }

    #[test]
    fn markdown_analysis_restores_common_metadata() {
        let path = unique_path("readme", "md");
        fs::write(&path, "# demo\n").expect("write markdown");

        let analysis = analyze_file(&path).expect("analyze file");
        assert_eq!(analysis.mime_type, "text/markdown");
        assert_eq!(analysis.extension, "md");
        assert_eq!(analysis.file_signature, "Markdown Document");
        assert_ne!(analysis.md5_state, Md5State::LargeFile);

        fs::remove_file(path).expect("cleanup markdown");
    }

    #[test]
    fn large_file_skips_md5_computation() {
        let path = unique_path("large", "bin");
        fs::write(&path, vec![b'x'; HASH_LIMIT as usize + 1]).expect("write large file");

        let analysis = analyze_file(&path).expect("analyze large file");
        assert_eq!(analysis.md5_state, Md5State::LargeFile);
        assert!(analysis.md5_hash.is_none());

        fs::remove_file(path).expect("cleanup large file");
    }
}
