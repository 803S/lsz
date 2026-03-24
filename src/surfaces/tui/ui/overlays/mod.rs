pub mod confirm;
pub mod help;
pub mod note_editor;
pub mod picker;
pub mod reader;

use crate::surfaces::tui::app::state::{AppState, OverlayState};
use ratatui::Frame;

pub fn render(frame: &mut Frame, state: &AppState) {
    match &state.overlay {
        OverlayState::None => {}
        OverlayState::Confirm { .. } => confirm::render(frame, state),
        OverlayState::Help { .. } => help::render(frame, state),
        OverlayState::NoteEditor { .. } => note_editor::render(frame, state),
        OverlayState::BookmarkPicker { .. } => picker::render(frame, state),
        OverlayState::Reader => reader::render(frame, state),
    }
}
