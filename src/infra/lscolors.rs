use std::{collections::HashMap, path::Path};

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
                let colored = format!("\x1b[{value}m");
                match key {
                    "fi" => default_file = colored,
                    "di" => default_dir = colored,
                    "ln" => default_link = colored,
                    "ex" => default_executable = colored,
                    _ => {
                        codes.insert(key.to_string(), colored);
                    }
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

    fn matches_pattern(&self, name: &str, path: &Path, pattern: &str) -> bool {
        if let Some(ext) = pattern.strip_prefix("*.") {
            return path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case(ext));
        }
        if let Some(suffix) = pattern.strip_prefix('*') {
            return name.to_lowercase().ends_with(&suffix.to_lowercase());
        }
        name.eq_ignore_ascii_case(pattern)
    }
}

impl Default for LsColors {
    fn default() -> Self {
        Self::from_env()
    }
}
