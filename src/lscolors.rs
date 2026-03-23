use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct LsColors {
    codes: HashMap<String, String>,
    default_file: String,
    default_dir: String,
    default_link: String,
    default_executable: String,
}

impl LsColors {
    pub fn from_env() -> Self {
        let ls_colors = std::env::var("LS_COLORS").unwrap_or_default();

        let mut codes = HashMap::new();
        let mut default_file = "\x1b[0m".to_string();
        let mut default_dir = "\x1b[01;34m".to_string();
        let mut default_link = "\x1b[01;36m".to_string();
        let mut default_executable = "\x1b[01;32m".to_string();

        for part in ls_colors.split(':') {
            if let Some((key, value)) = part.split_once('=') {
                let colored = format!("\x1b[{}m", value);
                if key == "fi" {
                    default_file = colored;
                } else if key == "di" {
                    default_dir = colored;
                } else if key == "ln" {
                    default_link = colored;
                } else if key == "ex" {
                    default_executable = colored;
                } else {
                    codes.insert(key.to_string(), colored);
                }
            }
        }

        Self {
            codes,
            default_file,
            default_dir,
            default_link,
            default_executable,
        }
    }

    pub fn for_path(
        &self,
        name: &str,
        is_dir: bool,
        is_symlink: bool,
        is_executable: bool,
    ) -> String {
        if is_dir {
            return self.default_dir.clone();
        }
        if is_symlink {
            return self.default_link.clone();
        }
        if is_executable {
            return self.default_executable.clone();
        }

        let path = Path::new(name);

        for (pattern, code) in &self.codes {
            if self.matches_pattern(name, path, pattern) {
                return code.clone();
            }
        }

        self.default_file.clone()
    }

    fn matches_pattern(&self, name: &str, path: &std::path::Path, pattern: &str) -> bool {
        if pattern.starts_with("*.") {
            let ext = &pattern[2..];
            if let Some(actual_ext) = path.extension().and_then(|e| e.to_str()) {
                return actual_ext.eq_ignore_ascii_case(ext);
            }
        } else if pattern.starts_with("*)") {
            let ext = &pattern[2..];
            if let Some(actual_ext) = path.extension().and_then(|e| e.to_str()) {
                return actual_ext.eq_ignore_ascii_case(ext);
            }
            return name.eq_ignore_ascii_case(&pattern[2..]);
        } else if pattern.starts_with("*") {
            let suffix = &pattern[1..];
            return name.to_lowercase().ends_with(&suffix.to_lowercase());
        } else if pattern.ends_with('*') {
            let prefix = &pattern[..pattern.len() - 1];
            return name.to_lowercase().starts_with(&prefix.to_lowercase());
        } else if pattern.starts_with('.') && pattern.contains(".*") {
            let base = pattern.trim_start_matches('.');
            let base = base.trim_start_matches("*.");
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                return ext.eq_ignore_ascii_case(base);
            }
        } else if pattern.starts_with("^") && pattern.ends_with('$') {
            let middle = &pattern[1..pattern.len() - 1];
            return name.eq_ignore_ascii_case(middle);
        } else {
            return name.eq_ignore_ascii_case(pattern);
        }

        false
    }
}

impl Default for LsColors {
    fn default() -> Self {
        Self::from_env()
    }
}
