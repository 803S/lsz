pub mod commandline;
pub mod explorer;
pub mod inspector;
pub mod layout;
pub mod overlays;
pub mod preview;
pub mod statusline;
pub mod theme;

use crate::{
    domain::preview::{PreviewColor, PreviewDocument, PreviewKind, PreviewLine},
    surfaces::tui::app::state::{AppState, FocusPane},
};
use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

pub fn render(frame: &mut Frame, state: &AppState) {
    let areas = layout::split(frame.size());
    match areas.mode {
        layout::LayoutMode::Wide => {
            explorer::render(frame, areas.explorer, state);
            preview::render(frame, areas.preview, state);
            inspector::render(frame, areas.inspector, state);
        }
        layout::LayoutMode::Stacked | layout::LayoutMode::Rows => {
            explorer::render(frame, areas.explorer, state);
            preview::render(frame, areas.preview, state);
            inspector::render(frame, areas.inspector, state);
        }
        layout::LayoutMode::Compact => {
            explorer::render(frame, areas.explorer, state);
            match state.focus {
                FocusPane::Inspector => inspector::render(frame, areas.inspector, state),
                _ => preview::render(frame, areas.preview, state),
            }
        }
        layout::LayoutMode::Micro => match state.focus {
            FocusPane::Explorer => explorer::render(frame, areas.explorer, state),
            FocusPane::Preview => preview::render(frame, areas.preview, state),
            FocusPane::Inspector => inspector::render(frame, areas.inspector, state),
        },
    }
    statusline::render(frame, areas.status, state);
    overlays::render(frame, state);
}

pub fn preview_line_to_tui(line: &PreviewLine) -> Line<'static> {
    let spans: Vec<Span<'static>> = line
        .spans
        .iter()
        .map(|span| {
            let mut style = Style::default();
            if let Some(color) = span.fg {
                style = style.fg(color_to_tui(color));
            }
            if span.bold {
                style = style.add_modifier(Modifier::BOLD);
            }
            if span.dim {
                style = style.add_modifier(Modifier::DIM);
            }
            Span::styled(span.text.clone(), style)
        })
        .collect();
    Line::from(spans)
}

pub fn preview_document_lines_to_tui(
    document: &PreviewDocument,
    show_line_numbers: bool,
) -> Vec<Line<'static>> {
    document
        .lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            if show_line_numbers && matches!(document.kind, PreviewKind::Code) {
                let mut spans = vec![Span::styled(
                    format!("{:>4} │ ", document.first_line_number + index),
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::DIM),
                )];
                spans.extend(line.spans.iter().map(|span| {
                    let mut style = Style::default();
                    if let Some(color) = span.fg {
                        style = style.fg(color_to_tui(color));
                    }
                    if span.bold {
                        style = style.add_modifier(Modifier::BOLD);
                    }
                    if span.dim {
                        style = style.add_modifier(Modifier::DIM);
                    }
                    Span::styled(span.text.clone(), style)
                }));
                Line::from(spans)
            } else {
                preview_line_to_tui(line)
            }
        })
        .collect()
}

fn color_to_tui(color: PreviewColor) -> Color {
    Color::Rgb(color.r, color.g, color.b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::preview::{PreviewDocument, PreviewLine, PreviewModel, PreviewWrapMode};

    fn code_document() -> PreviewDocument {
        PreviewDocument {
            kind: PreviewKind::Code,
            title: "demo.rs".to_string(),
            subtitle: None,
            provider_name: "文本".to_string(),
            degraded: false,
            metadata: Vec::new(),
            warnings: Vec::new(),
            actions: Vec::new(),
            wrap_mode: PreviewWrapMode::NoWrap,
            lines: vec![PreviewLine::plain("let value = 1;")],
            first_line_number: 1,
        }
    }

    fn line_text(line: &Line<'static>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }

    #[test]
    fn code_line_numbers_only_exist_in_render_layer() {
        let document = code_document();
        let with_line_numbers = preview_document_lines_to_tui(&document, true);
        let without_line_numbers = preview_document_lines_to_tui(&document, false);

        assert!(line_text(&with_line_numbers[0]).starts_with("   1 │ let value = 1;"));
        assert_eq!(line_text(&without_line_numbers[0]), "let value = 1;");
        assert!(!line_text(&without_line_numbers[0]).contains('│'));
    }

    #[test]
    fn source_preview_lines_keep_plain_text_without_ui_prefixes() {
        let document = code_document();
        match PreviewModel::Document(document.clone()) {
            PreviewModel::Document(doc) => {
                assert_eq!(doc.lines[0].spans[0].text, "let value = 1;");
            }
            _ => unreachable!("document expected"),
        }
    }
}
