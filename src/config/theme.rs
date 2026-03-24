use ratatui::style::Color;

#[derive(Debug, Clone)]
pub struct AppTheme {
    pub accent: Color,
    pub selection_bg: Color,
    pub muted: Color,
    pub warning: Color,
    pub success: Color,
    pub error: Color,
}

impl Default for AppTheme {
    fn default() -> Self {
        Self {
            accent: Color::Cyan,
            selection_bg: Color::DarkGray,
            muted: Color::DarkGray,
            warning: Color::Yellow,
            success: Color::Green,
            error: Color::Red,
        }
    }
}
