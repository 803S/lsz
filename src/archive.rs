use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

const MAX_DEPTH: usize = 3;
const MAX_ENTRIES: usize = 2000;

#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    pub name: String,
    pub size: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone)]
pub struct ArchivePreview {
    pub format: String,
    pub total_files: usize,
    pub total_size: String,
    pub hidden_count: usize,
    pub grouped: BTreeMap<String, Vec<ArchiveEntry>>,
    pub is_supported: bool,
    pub truncated: bool,
}

fn format_size(size: u64) -> String {
    if size < 1024 {
        format!("{}B", size)
    } else if size < 1024 * 1024 {
        format!("{:.1}KB", size as f64 / 1024.0)
    } else if size < 1024 * 1024 * 1024 {
        format!("{:.1}MB", size as f64 / 1024.0 / 1024.0)
    } else {
        format!("{:.1}GB", size as f64 / 1024.0 / 1024.0 / 1024.0)
    }
}

fn get_depth(path: &str) -> usize {
    path.trim_matches('/').matches('/').count()
}

fn build_preview(
    entries: Vec<(String, u64, bool)>,
    format: String,
    total_count: usize,
) -> ArchivePreview {
    let total_size: u64 = entries.iter().map(|(_, s, _)| *s).sum();
    let hidden_count = total_count.saturating_sub(entries.len());
    let mut grouped = BTreeMap::new();

    for (full_path, size, is_dir) in entries.into_iter().take(MAX_ENTRIES) {
        if get_depth(&full_path) > MAX_DEPTH {
            continue;
        }
        let path = Path::new(&full_path);
        let parent = path
            .parent()
            .map(|p| {
                let s = p.to_string_lossy();
                if s.is_empty() {
                    "/".to_string()
                } else {
                    format!("{}/", s)
                }
            })
            .unwrap_or_else(|| "/".to_string());
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| full_path.clone());
        grouped
            .entry(parent)
            .or_insert_with(Vec::new)
            .push(ArchiveEntry {
                name,
                size: format_size(size),
                is_dir,
            });
    }

    ArchivePreview {
        format,
        total_files: total_count,
        total_size: format_size(total_size),
        hidden_count,
        grouped,
        is_supported: true,
        truncated: total_count > MAX_ENTRIES,
    }
}

pub fn preview_archive<P: AsRef<Path>>(path: P) -> std::io::Result<ArchivePreview> {
    let path = path.as_ref();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "zip" => preview_zip(path),
        "tar" | "gz" | "tgz" => preview_tar_like(path),
        "7z" => preview_7z(path),
        _ => Ok(ArchivePreview {
            format: format!("{} Archive", ext.to_uppercase()),
            total_files: 0,
            total_size: "-".to_string(),
            hidden_count: 0,
            grouped: BTreeMap::new(),
            is_supported: false,
            truncated: false,
        }),
    }
}

fn preview_zip(path: &Path) -> std::io::Result<ArchivePreview> {
    let output = Command::new("zipinfo")
        .args(["-l", path.to_str().unwrap_or("")])
        .output()?;
    if !output.status.success() {
        return Ok(ArchivePreview {
            format: "ZIP Archive".to_string(),
            total_files: 0,
            total_size: "-".to_string(),
            hidden_count: 0,
            grouped: BTreeMap::new(),
            is_supported: false,
            truncated: false,
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();
    for line in stdout.lines().skip(3) {
        if line.contains(" files") && line.contains(" bytes") {
            break;
        }
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 9 {
            continue;
        }
        let size = cols[3].parse::<u64>().unwrap_or(0);
        let name = cols[8..].join(" ");
        entries.push((name.clone(), size, name.ends_with('/')));
        if entries.len() >= MAX_ENTRIES {
            break;
        }
    }
    let total = entries.len();
    Ok(build_preview(entries, "ZIP Archive".to_string(), total))
}

fn preview_tar_like(path: &Path) -> std::io::Result<ArchivePreview> {
    let output = Command::new("tar")
        .args(["-tf", path.to_str().unwrap_or("")])
        .output()?;
    if !output.status.success() {
        return Ok(ArchivePreview {
            format: "TAR Archive".to_string(),
            total_files: 0,
            total_size: "-".to_string(),
            hidden_count: 0,
            grouped: BTreeMap::new(),
            is_supported: false,
            truncated: false,
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let entries: Vec<_> = stdout
        .lines()
        .take(MAX_ENTRIES)
        .map(|line| (line.to_string(), 0, line.ends_with('/')))
        .collect();
    let total = stdout.lines().count();
    Ok(build_preview(entries, "TAR Archive".to_string(), total))
}

fn preview_7z(path: &Path) -> std::io::Result<ArchivePreview> {
    let output = Command::new("timeout")
        .args(["5", "7z", "l", path.to_str().unwrap_or("")])
        .output()?;
    if !output.status.success() {
        return Ok(ArchivePreview {
            format: "7Z Archive".to_string(),
            total_files: 0,
            total_size: "-".to_string(),
            hidden_count: 0,
            grouped: BTreeMap::new(),
            is_supported: false,
            truncated: false,
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();
    for line in stdout.lines() {
        if !line.contains(' ') || line.starts_with("----") {
            continue;
        }
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 6 {
            continue;
        }
        let maybe_size = cols[3].parse::<u64>();
        if let Ok(size) = maybe_size {
            let name = cols[5..].join(" ");
            entries.push((name.clone(), size, false));
            if entries.len() >= MAX_ENTRIES {
                break;
            }
        }
    }
    let total = entries.len();
    Ok(build_preview(entries, "7Z Archive".to_string(), total))
}
