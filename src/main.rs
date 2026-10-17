mod app;
mod input;
mod proxy;
mod storage;
mod tui;
mod ui;

use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();

    // Modo auxiliar invocado por pkexec: solo aplica el cambio y termina.
    if args.get(1).map(String::as_str) == Some("-apply") {
        return run_apply_helper(&args[2..]);
    }

    match run_tui() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("proxy-manager: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run_apply_helper(args: &[String]) -> ExitCode {
    let mut pc = proxy::ProxyConfig::default();
    let mut file: Option<PathBuf> = None;
    let mut enable = true;

    for arg in args {
        let Some((key, value)) = arg.split_once('=') else {
            continue;
        };
        match key {
            "-scheme" => pc.scheme = value.to_string(),
            "-user" => pc.user = value.to_string(),
            "-pass" => pc.pass = value.to_string(),
            "-url" => pc.host = value.to_string(),
            "-port" => pc.port = value.parse().unwrap_or(0),
            "-noproxy" => pc.no_proxy = value.to_string(),
            "-file" => file = Some(PathBuf::from(value)),
            "-enable" => enable = value == "true",
            _ => {}
        }
    }

    let Some(file) = file else {
        return ExitCode::SUCCESS;
    };
    match proxy::change_proxy_config(&pc, &file, enable) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!(
                "proxy-manager: no se pudo escribir {}: {err}",
                file.display()
            );
            ExitCode::FAILURE
        }
    }
}

fn run_tui() -> io::Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(io::Error::other(
            "esta aplicación necesita una terminal (TUI); ejecútala desde tu terminal",
        ));
    }

    let targets = proxy::Targets::new(proxy::shell_config_path());

    // Asegura que las variables de proxy existan antes de dibujar la TUI
    // (pkexec puede pedir la contraseña en la terminal).
    proxy::ensure_proxy_vars_exist(&targets);

    let settings = storage::load_settings();
    let mut app = app::App::new(targets, settings);

    let mut terminal = tui::enter()?;
    let result = run_event_loop(&mut terminal, &mut app);
    tui::leave(&mut terminal)?;
    app.persist_settings();
    result
}

fn run_event_loop(terminal: &mut ratatui::DefaultTerminal, app: &mut app::App) -> io::Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyEventKind};

    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;
        if app.should_quit {
            break;
        }
        match event::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                app.handle_key(key, terminal)?;
            }
            Event::Paste(text) => app.handle_paste(&text),
            _ => {}
        }
    }
    Ok(())
}
