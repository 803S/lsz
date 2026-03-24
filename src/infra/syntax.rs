use crate::domain::preview::{PreviewColor, PreviewLine, PreviewSpan};
use std::{path::Path, sync::OnceLock};
use syntect::{
    easy::HighlightLines,
    highlighting::{Theme, ThemeSet},
    parsing::SyntaxReference,
    parsing::SyntaxSet,
    util::LinesWithEndings,
};

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();

fn syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn theme() -> &'static Theme {
    let themes = THEME_SET.get_or_init(ThemeSet::load_defaults);
    themes
        .themes
        .get("base16-mocha.dark")
        .or_else(|| themes.themes.get("base16-eighties.dark"))
        .or_else(|| themes.themes.get("base16-ocean.dark"))
        .or_else(|| themes.themes.get("Solarized (dark)"))
        .or_else(|| themes.themes.get("InspiredGitHub"))
        .or_else(|| themes.themes.values().next())
        .expect("syntect default themes should not be empty")
}

pub fn highlight_code(path: &Path, content: &str, max_lines: usize) -> Vec<PreviewLine> {
    highlight_code_with_hint(path, None, content, max_lines)
}

pub fn highlight_code_with_hint(
    path: &Path,
    syntax_hint: Option<&str>,
    content: &str,
    max_lines: usize,
) -> Vec<PreviewLine> {
    let syntax = resolve_syntax(path, syntax_hint, content);
    let mut highlighter = HighlightLines::new(syntax, theme());
    let mut lines = Vec::new();
    for raw_line in LinesWithEndings::from(content).take(max_lines) {
        let line = raw_line.trim_end_matches(['\r', '\n']);
        let mut spans = Vec::new();
        if let Ok(ranges) = highlighter.highlight_line(raw_line, syntax_set()) {
            for (style, text) in ranges {
                let text = text.trim_end_matches(['\r', '\n']);
                if text.is_empty() {
                    continue;
                }
                spans.push(PreviewSpan {
                    text: text.to_string(),
                    fg: Some(adjust_color(
                        style.foreground.r,
                        style.foreground.g,
                        style.foreground.b,
                    )),
                    bold: style
                        .font_style
                        .contains(syntect::highlighting::FontStyle::BOLD),
                    dim: false,
                });
            }
        } else {
            spans.push(PreviewSpan {
                text: line.to_string(),
                fg: None,
                bold: false,
                dim: false,
            });
        }
        lines.push(PreviewLine { spans });
    }
    lines
}

fn adjust_color(r: u8, g: u8, b: u8) -> PreviewColor {
    let (hue, saturation, value) = rgb_to_hsv(r, g, b);
    if saturation < 0.14 {
        let gray = ((value * 255.0) * 0.78 + 42.0).clamp(110.0, 220.0).round() as u8;
        return PreviewColor {
            r: gray,
            g: gray,
            b: gray,
        };
    }

    let palette = match hue {
        hue if !(18.0..345.0).contains(&hue) => (237.0, 135.0, 150.0),
        hue if hue < 42.0 => (245.0, 169.0, 127.0),
        hue if hue < 68.0 => (238.0, 212.0, 159.0),
        hue if hue < 160.0 => (166.0, 218.0, 149.0),
        hue if hue < 215.0 => (145.0, 215.0, 227.0),
        hue if hue < 255.0 => (138.0, 173.0, 244.0),
        hue if hue < 315.0 => (198.0, 160.0, 246.0),
        _ => (245.0, 169.0, 127.0),
    };
    let boost = (0.72 + saturation * 0.18).clamp(0.72, 0.92);
    let brightness = (0.88 + value * 0.18).clamp(0.88, 1.0);
    let mix = |base: f32, original: u8| {
        ((base * boost + original as f32 * (1.0 - boost)) * brightness)
            .clamp(0.0, 255.0)
            .round() as u8
    };

    PreviewColor {
        r: mix(palette.0, r),
        g: mix(palette.1, g),
        b: mix(palette.2, b),
    }
}

fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let hue = if delta <= f32::EPSILON {
        0.0
    } else if (max - r).abs() <= f32::EPSILON {
        60.0 * (((g - b) / delta).rem_euclid(6.0))
    } else if (max - g).abs() <= f32::EPSILON {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };
    let saturation = if max <= f32::EPSILON {
        0.0
    } else {
        delta / max
    };
    (hue, saturation, max)
}

fn resolve_syntax<'a>(path: &Path, syntax_hint: Option<&str>, content: &str) -> &'a SyntaxReference
where
    'static: 'a,
{
    if let Some(syntax) = syntax_hint.and_then(find_syntax_from_hint) {
        return syntax;
    }
    if let Some(file_name) = path.file_name().and_then(|value| value.to_str()) {
        let lower = file_name.to_ascii_lowercase();
        if let Some(syntax) = match lower.as_str() {
            "dockerfile" => find_syntax_from_hint("dockerfile"),
            "makefile" => find_syntax_from_hint("makefile"),
            "justfile" => find_syntax_from_hint("justfile"),
            ".env" | ".bashrc" | ".zshrc" | ".profile" | ".bash_profile" | ".bash_aliases" => {
                find_syntax_from_hint("sh")
            }
            "gemfile" | "rakefile" => find_syntax_from_hint("ruby"),
            "cmakelists.txt" => find_syntax_from_hint("cmake"),
            "powershell_profile.ps1" => find_syntax_from_hint("powershell"),
            _ => None,
        } {
            return syntax;
        }
    }
    if let Some(syntax) = path
        .extension()
        .and_then(|value| value.to_str())
        .and_then(find_syntax_from_hint)
    {
        return syntax;
    }
    if let Some(first_line) = content.lines().next() {
        if let Some(syntax) = syntax_set().find_syntax_by_first_line(first_line) {
            return syntax;
        }
    }
    syntax_set().find_syntax_plain_text()
}

fn find_syntax_from_hint(hint: &str) -> Option<&'static SyntaxReference> {
    let normalized = hint.trim().trim_matches('{').trim_matches('}');
    let normalized = normalized.to_ascii_lowercase();
    let (ext_candidates, name_candidates) = match normalized.as_str() {
        "shell" | "sh" | "bash" | "zsh" | "fish" => (
            vec!["sh", "bash", "zsh", "fish"],
            vec!["Shell-Unix-Generic"],
        ),
        "py" | "python" => (vec!["py"], vec!["Python"]),
        "javascript" | "js" | "mjs" | "cjs" | "node" => {
            (vec!["js", "mjs", "cjs"], vec!["JavaScript"])
        }
        "typescript" | "ts" => (vec!["ts"], vec!["TypeScript"]),
        "tsx" => (vec!["tsx"], vec!["TypeScriptReact"]),
        "jsx" => (vec!["jsx"], vec!["JavaScript (Babel)"]),
        "yaml" | "yml" => (vec!["yaml", "yml"], vec!["YAML"]),
        "dockerfile" => (vec!["dockerfile"], vec!["Dockerfile"]),
        "makefile" => (vec!["makefile"], vec!["Makefile"]),
        "justfile" => (vec!["just"], vec!["Makefile"]),
        "toml" => (vec!["toml"], vec!["TOML"]),
        "json" => (vec!["json"], vec!["JSON"]),
        "conf" | "config" => (vec!["conf", "cfg", "ini"], vec!["INI"]),
        "php" | "phtml" => (vec!["php", "phtml"], vec!["PHP"]),
        "ruby" | "rb" | "gemspec" | "rake" => (vec!["rb"], vec!["Ruby"]),
        "rust" | "rs" => (vec!["rs"], vec!["Rust"]),
        "c" => (vec!["c", "h"], vec!["C"]),
        "c++" | "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" => {
            (vec!["cpp", "cxx", "cc", "hpp", "hxx", "hh"], vec!["C++"])
        }
        "powershell" | "pwsh" | "ps1" | "psm1" | "psd1" => (
            vec!["ps1", "psm1", "psd1", "sh"],
            vec!["PowerShell", "Shell-Unix-Generic"],
        ),
        "cmake" => (vec!["cmake"], vec!["CMake"]),
        "html" => (vec!["html"], vec!["HTML"]),
        "css" => (vec!["css"], vec!["CSS"]),
        "sql" => (vec!["sql"], vec!["SQL"]),
        other => (vec![other], vec![other]),
    };
    for ext in ext_candidates {
        if let Some(syntax) = syntax_set().find_syntax_by_extension(ext) {
            return Some(syntax);
        }
    }
    for name in name_candidates {
        if let Some(syntax) = syntax_set().find_syntax_by_name(name) {
            return Some(syntax);
        }
    }
    syntax_set().find_syntax_by_name(&normalized)
}

#[cfg(test)]
mod tests {
    use super::{highlight_code, resolve_syntax, syntax_set};
    use std::path::Path;

    #[test]
    fn highlighted_lines_strip_line_endings() {
        let lines = highlight_code(
            Path::new("demo.rs"),
            "fn main() {}\r\nprintln!(\"hi\");\n",
            20,
        );
        assert!(lines.iter().all(|line| {
            line.spans
                .iter()
                .all(|span| !span.text.contains('\n') && !span.text.contains('\r'))
        }));
    }

    #[test]
    fn highlighted_code_uses_multiple_vivid_colors() {
        let lines = highlight_code(
            Path::new("demo.rs"),
            "fn main() {\n    let value = String::from(\"hi\");\n    println!(\"{value}\");\n}\n",
            20,
        );

        let vivid_colors = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .filter_map(|span| span.fg)
            .filter(|color| {
                let max = color.r.max(color.g).max(color.b);
                let min = color.r.min(color.g).min(color.b);
                max.saturating_sub(min) >= 12
            })
            .count();

        assert!(vivid_colors >= 3);
    }

    #[test]
    fn resolve_syntax_supports_more_languages() {
        let plain = syntax_set().find_syntax_plain_text().name.as_str();

        let python = resolve_syntax(
            Path::new("demo.py"),
            None,
            "import os\nclass Demo:\n    pass\n",
        );
        let php = resolve_syntax(Path::new("demo.php"), None, "<?php echo 1; ?>\n");
        let powershell = resolve_syntax(
            Path::new("demo.ps1"),
            None,
            "Write-Host 'hello'\nGet-ChildItem\n",
        );
        let shell = resolve_syntax(Path::new("script"), None, "#!/usr/bin/env bash\necho hi\n");

        assert_ne!(python.name, plain);
        assert_ne!(php.name, plain);
        assert_ne!(powershell.name, plain);
        assert_ne!(shell.name, plain);
    }

    #[test]
    fn python_highlight_separates_keywords_strings_and_comments() {
        let lines = highlight_code(
            Path::new("main.py"),
            "import os\nclass Demo:\n    def run(self):\n        value = \"hello\"\n        # note\n        return value\n",
            20,
        );

        let mut string_color = None;
        let mut comment_color = None;
        let mut keyword_color = None;
        for line in &lines {
            for span in &line.spans {
                if span.text.contains("import") {
                    keyword_color = span.fg;
                }
                if span.text.contains("hello") {
                    string_color = span.fg;
                }
                if span.text.contains("# note") || span.text.contains("note") {
                    comment_color = span.fg;
                }
            }
        }

        let keyword_color = keyword_color.expect("keyword color");
        let string_color = string_color.expect("string color");
        let comment_color = comment_color.expect("comment color");

        assert_ne!(
            (keyword_color.r, keyword_color.g, keyword_color.b),
            (string_color.r, string_color.g, string_color.b)
        );
        assert_ne!(
            (string_color.r, string_color.g, string_color.b),
            (comment_color.r, comment_color.g, comment_color.b)
        );
    }

    #[test]
    fn python_single_line_comment_does_not_leak_to_following_lines() {
        let lines = highlight_code(
            Path::new("database.py"),
            "value = 1\n# 注释\nnext_value = 2\n",
            20,
        );

        let comment_color = lines[1]
            .spans
            .iter()
            .find(|span| span.text.contains("注释"))
            .and_then(|span| span.fg)
            .expect("comment color");
        let code_color = lines[2]
            .spans
            .iter()
            .find(|span| span.text.contains("next_value"))
            .and_then(|span| span.fg)
            .expect("code color");

        assert_ne!(
            (comment_color.r, comment_color.g, comment_color.b),
            (code_color.r, code_color.g, code_color.b)
        );
    }
}
