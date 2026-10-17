use std::io::{self, stdout};

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{DisableBracketedPaste, EnableBracketedPaste};
use ratatui::crossterm::execute;

/// Inicia la TUI (pantalla alternativa, modo raw, pegado entre corchetes).
pub fn enter() -> io::Result<DefaultTerminal> {
    let terminal = ratatui::init();
    execute!(stdout(), EnableBracketedPaste)?;
    Ok(terminal)
}

/// Restaura la terminal al estado normal.
pub fn leave(_terminal: &mut DefaultTerminal) -> io::Result<()> {
    let _ = execute!(stdout(), DisableBracketedPaste);
    ratatui::restore();
    Ok(())
}

/// Vuelve a entrar en la TUI sobre un terminal ya existente (tras suspenderla
/// para pedir permisos con pkexec).
pub fn enter_existing(terminal: &mut DefaultTerminal) -> io::Result<()> {
    *terminal = ratatui::init();
    execute!(stdout(), EnableBracketedPaste)?;
    terminal.clear()
}
