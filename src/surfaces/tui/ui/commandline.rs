use crate::surfaces::tui::app::state::AppState;

pub fn current_prompt(state: &AppState) -> String {
    match state.input_mode {
        crate::surfaces::tui::app::state::InputMode::Search => format!("/{}", state.search_input),
        crate::surfaces::tui::app::state::InputMode::Command => {
            if state.command_input.is_empty() {
                ":".to_string()
            } else {
                format!(":{}", state.command_input)
            }
        }
        crate::surfaces::tui::app::state::InputMode::Normal => String::new(),
    }
}
