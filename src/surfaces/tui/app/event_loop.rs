use crate::surfaces::tui::{
    app::{
        action::{Action, InputEdit},
        effect::perform,
        event::AppEvent,
        reducer::reduce,
        state::{AppState, InputMode, OverlayState},
    },
    ui,
};
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    time::Duration,
};

type AppTerminal = Terminal<CrosstermBackend<std::io::Stdout>>;

struct TerminalGuard {
    terminal: AppTerminal,
}

impl TerminalGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

pub fn run(initial_cwd: PathBuf, initial_selection: Option<PathBuf>) -> Result<()> {
    let mut terminal = TerminalGuard::enter()?;
    let (tx, rx): (Sender<AppEvent>, Receiver<AppEvent>) = mpsc::channel();

    let mut state = AppState::new(initial_cwd.clone(), initial_selection);
    dispatch(&mut state, &tx, Action::LoadDir(initial_cwd));

    let mut should_quit = false;
    while !should_quit {
        terminal.terminal.draw(|frame| ui::render(frame, &state))?;

        while let Ok(event) = rx.try_recv() {
            should_quit |= handle_app_event(&mut state, &tx, event);
        }

        if event::poll(Duration::from_millis(100))? {
            let event = event::read()?;
            should_quit |= handle_input_event(&mut state, &tx, event);
        }
    }
    Ok(())
}

fn handle_app_event(state: &mut AppState, tx: &Sender<AppEvent>, event: AppEvent) -> bool {
    match event {
        AppEvent::DirLoaded { cwd, entries } => {
            dispatch(state, tx, Action::DirLoaded { cwd, entries })
        }
        AppEvent::DirFailed(message) => dispatch(state, tx, Action::DirFailed(message)),
        AppEvent::PreviewLoaded { path, preview } => {
            dispatch(state, tx, Action::PreviewLoaded { path, preview })
        }
        AppEvent::BookmarksLoaded(bookmarks) => {
            dispatch(state, tx, Action::BookmarksLoaded(bookmarks))
        }
        AppEvent::Status(message) => dispatch(state, tx, Action::Status(message)),
    }
}

fn dispatch(state: &mut AppState, tx: &Sender<AppEvent>, action: Action) -> bool {
    let result = reduce(state, action);
    for effect in result.effects {
        perform(effect, tx.clone());
    }
    result.should_quit
}

fn handle_input_event(state: &mut AppState, tx: &Sender<AppEvent>, event: Event) -> bool {
    match &state.overlay {
        OverlayState::Help { .. } => return handle_help_input(state, tx, event),
        OverlayState::NoteEditor { .. } => return handle_note_editor_input(state, tx, event),
        OverlayState::BookmarkPicker { .. } => {
            return handle_bookmark_picker_input(state, tx, event);
        }
        OverlayState::Confirm { .. } => return handle_confirm_input(state, tx, event),
        OverlayState::Reader => return handle_reader_input(state, tx, event),
        OverlayState::None => {}
    }

    if state.input_mode == InputMode::Search {
        return handle_search_input(state, tx, event);
    }
    if state.input_mode == InputMode::Command {
        return handle_command_input(state, tx, event);
    }

    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
            KeyCode::Char('q') => dispatch(state, tx, Action::Quit),
            KeyCode::Char('j') | KeyCode::Down => match state.focus {
                crate::surfaces::tui::app::state::FocusPane::Explorer => {
                    dispatch(state, tx, Action::MoveDown)
                }
                _ => dispatch(state, tx, Action::ScrollPreview(1)),
            },
            KeyCode::Char('k') | KeyCode::Up => match state.focus {
                crate::surfaces::tui::app::state::FocusPane::Explorer => {
                    dispatch(state, tx, Action::MoveUp)
                }
                _ => dispatch(state, tx, Action::ScrollPreview(-1)),
            },
            KeyCode::Enter => match state.focus {
                crate::surfaces::tui::app::state::FocusPane::Explorer => {
                    dispatch(state, tx, Action::OpenSelected)
                }
                _ => dispatch(state, tx, Action::OpenReader),
            },
            KeyCode::Backspace => dispatch(state, tx, Action::GoParent),
            KeyCode::Char('h') | KeyCode::Left => match state.focus {
                crate::surfaces::tui::app::state::FocusPane::Explorer => {
                    dispatch(state, tx, Action::GoParent)
                }
                _ => dispatch(state, tx, Action::FocusExplorer),
            },
            KeyCode::Esc if !state.search_input.is_empty() => {
                dispatch(state, tx, Action::ClearSearch)
            }
            KeyCode::Char('/') => dispatch(state, tx, Action::OpenSearch),
            KeyCode::Char(':') => dispatch(state, tx, Action::OpenCommand),
            KeyCode::Tab => dispatch(state, tx, Action::FocusNext),
            KeyCode::Char('?') | KeyCode::F(1) => dispatch(state, tx, Action::OpenHelp),
            KeyCode::Char('m') => dispatch(state, tx, Action::OpenNoteEditor),
            KeyCode::Char('p') => dispatch(state, tx, Action::ToggleBookmark),
            KeyCode::Char('B') => dispatch(state, tx, Action::OpenBookmarkPicker),
            KeyCode::Char('o') => dispatch(state, tx, Action::OpenExternal),
            KeyCode::Char('r') => dispatch(state, tx, Action::Refresh),
            KeyCode::Char('.') => dispatch(state, tx, Action::ToggleHidden),
            KeyCode::Char('n')
                if state.focus != crate::surfaces::tui::app::state::FocusPane::Explorer =>
            {
                dispatch(state, tx, Action::ToggleLineNumbers)
            }
            KeyCode::Char('H')
                if state.focus == crate::surfaces::tui::app::state::FocusPane::Preview =>
            {
                dispatch(state, tx, Action::ScrollPreviewHorizontal(-4))
            }
            KeyCode::Char('L')
                if state.focus == crate::surfaces::tui::app::state::FocusPane::Preview =>
            {
                dispatch(state, tx, Action::ScrollPreviewHorizontal(4))
            }
            KeyCode::PageDown => dispatch(state, tx, Action::ScrollPreview(10)),
            KeyCode::PageUp => dispatch(state, tx, Action::ScrollPreview(-10)),
            _ => false,
        },
        Event::Mouse(mouse) => match mouse.kind {
            MouseEventKind::ScrollDown => dispatch(state, tx, Action::ScrollPreview(3)),
            MouseEventKind::ScrollUp => dispatch(state, tx, Action::ScrollPreview(-3)),
            _ => false,
        },
        _ => false,
    }
}

fn handle_search_input(state: &mut AppState, tx: &Sender<AppEvent>, event: Event) -> bool {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
            KeyCode::Esc => dispatch(state, tx, Action::ClearSearch),
            KeyCode::Enter => dispatch(state, tx, Action::SubmitSearch),
            KeyCode::Char('?') | KeyCode::F(1) => dispatch(state, tx, Action::OpenHelp),
            KeyCode::Backspace => dispatch(state, tx, Action::SearchInput(InputEdit::Backspace)),
            KeyCode::Char(ch) => dispatch(state, tx, Action::SearchInput(InputEdit::Insert(ch))),
            _ => false,
        },
        _ => false,
    }
}

fn handle_command_input(state: &mut AppState, tx: &Sender<AppEvent>, event: Event) -> bool {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
            KeyCode::Esc => {
                state.input_mode = InputMode::Normal;
                state.command_input.clear();
                false
            }
            KeyCode::Char('?') | KeyCode::F(1) => dispatch(state, tx, Action::OpenHelp),
            KeyCode::Enter => dispatch(state, tx, Action::SubmitCommand),
            KeyCode::Backspace => dispatch(state, tx, Action::CommandInput(InputEdit::Backspace)),
            KeyCode::Char(ch) => dispatch(state, tx, Action::CommandInput(InputEdit::Insert(ch))),
            _ => false,
        },
        _ => false,
    }
}

fn handle_help_input(state: &mut AppState, tx: &Sender<AppEvent>, event: Event) -> bool {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') | KeyCode::F(1) => {
                dispatch(state, tx, Action::CloseOverlay)
            }
            KeyCode::Tab | KeyCode::Right => dispatch(state, tx, Action::HelpNextCategory),
            KeyCode::BackTab | KeyCode::Left => dispatch(state, tx, Action::HelpPrevCategory),
            KeyCode::Down => dispatch(state, tx, Action::HelpScroll(1)),
            KeyCode::Up => dispatch(state, tx, Action::HelpScroll(-1)),
            KeyCode::PageDown => dispatch(state, tx, Action::HelpScroll(8)),
            KeyCode::PageUp => dispatch(state, tx, Action::HelpScroll(-8)),
            KeyCode::Backspace => dispatch(state, tx, Action::HelpInput(InputEdit::Backspace)),
            KeyCode::Char(ch) => dispatch(state, tx, Action::HelpInput(InputEdit::Insert(ch))),
            _ => false,
        },
        _ => false,
    }
}

fn handle_note_editor_input(state: &mut AppState, tx: &Sender<AppEvent>, event: Event) -> bool {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
            KeyCode::Esc => dispatch(state, tx, Action::CloseOverlay),
            KeyCode::Enter => dispatch(state, tx, Action::SaveNote),
            KeyCode::F(1) | KeyCode::Char('?') => dispatch(state, tx, Action::OpenHelp),
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                dispatch(state, tx, Action::NoteClearLine)
            }
            KeyCode::Backspace => dispatch(state, tx, Action::NoteInput(InputEdit::Backspace)),
            KeyCode::Char(ch) => dispatch(state, tx, Action::NoteInput(InputEdit::Insert(ch))),
            _ => false,
        },
        _ => false,
    }
}

fn handle_bookmark_picker_input(state: &mut AppState, tx: &Sender<AppEvent>, event: Event) -> bool {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
            KeyCode::Esc => dispatch(state, tx, Action::CloseOverlay),
            KeyCode::Enter => dispatch(state, tx, Action::ConfirmBookmark),
            KeyCode::F(1) | KeyCode::Char('?') => dispatch(state, tx, Action::OpenHelp),
            KeyCode::Char('j') | KeyCode::Down => dispatch(state, tx, Action::BookmarkNext),
            KeyCode::Char('k') | KeyCode::Up => dispatch(state, tx, Action::BookmarkPrev),
            KeyCode::Char('d') => dispatch(state, tx, Action::DeleteBookmark),
            _ => false,
        },
        _ => false,
    }
}

fn handle_confirm_input(state: &mut AppState, tx: &Sender<AppEvent>, event: Event) -> bool {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
            KeyCode::Esc | KeyCode::Char('n') => dispatch(state, tx, Action::CloseOverlay),
            KeyCode::Enter | KeyCode::Char('y') => dispatch(state, tx, Action::ConfirmAccept),
            KeyCode::F(1) | KeyCode::Char('?') => dispatch(state, tx, Action::OpenHelp),
            _ => false,
        },
        _ => false,
    }
}

fn handle_reader_input(state: &mut AppState, tx: &Sender<AppEvent>, event: Event) -> bool {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => {
                dispatch(state, tx, Action::CloseOverlay)
            }
            KeyCode::F(1) | KeyCode::Char('?') => dispatch(state, tx, Action::OpenHelp),
            KeyCode::Char('n') => dispatch(state, tx, Action::ToggleLineNumbers),
            KeyCode::Left | KeyCode::Char('H') => {
                dispatch(state, tx, Action::ScrollPreviewHorizontal(-4))
            }
            KeyCode::Right | KeyCode::Char('L') => {
                dispatch(state, tx, Action::ScrollPreviewHorizontal(4))
            }
            KeyCode::Char('j') | KeyCode::Down => dispatch(state, tx, Action::ScrollPreview(1)),
            KeyCode::Char('k') | KeyCode::Up => dispatch(state, tx, Action::ScrollPreview(-1)),
            KeyCode::PageDown => dispatch(state, tx, Action::ScrollPreview(8)),
            KeyCode::PageUp => dispatch(state, tx, Action::ScrollPreview(-8)),
            KeyCode::Char('o') => dispatch(state, tx, Action::OpenExternal),
            _ => false,
        },
        Event::Mouse(mouse) => match mouse.kind {
            MouseEventKind::ScrollDown => dispatch(state, tx, Action::ScrollPreview(3)),
            MouseEventKind::ScrollUp => dispatch(state, tx, Action::ScrollPreview(-3)),
            _ => false,
        },
        _ => false,
    }
}
