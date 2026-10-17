use std::io;
use std::path::Path;
use std::process::Command;

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::ListState;

use crate::input::Input;
use crate::proxy::{
    ProxyConfig, Targets, build_url, change_proxy_config, is_proxy_enabled, parse_config_from_file,
};
use crate::storage::{
    HistoryEntry, Settings, add_history_entry, load_history, save_history, save_settings,
};
use crate::tui;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    History,
    Url,
    NoProxy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MsgKind {
    Info,
    Success,
    Error,
}

pub struct App {
    pub targets: Targets,
    pub url: Input,
    pub no_proxy: Input,
    pub history: Vec<HistoryEntry>,
    pub hist_state: ListState,
    pub enabled: bool,
    pub focus: Focus,
    pub split: f64,
    pub settings: Settings,
    pub message: String,
    pub msg_kind: MsgKind,
    pub show_help: bool,
    pub should_quit: bool,
}

impl App {
    pub fn new(targets: Targets, settings: Settings) -> Self {
        let mut app = Self {
            targets,
            url: Input::default(),
            no_proxy: Input::default(),
            history: load_history(),
            hist_state: ListState::default(),
            enabled: false,
            focus: Focus::History,
            split: settings.split_offset.clamp(0.15, 0.85),
            settings,
            message: String::new(),
            msg_kind: MsgKind::Info,
            show_help: false,
            should_quit: false,
        };
        app.reload_form_from_files();
        app.enabled = is_proxy_enabled(&app.targets.files());
        app
    }

    pub fn info(&mut self, msg: impl Into<String>) {
        self.message = msg.into();
        self.msg_kind = MsgKind::Info;
    }

    pub fn success(&mut self, msg: impl Into<String>) {
        self.message = msg.into();
        self.msg_kind = MsgKind::Success;
    }

    pub fn error(&mut self, msg: impl Into<String>) {
        self.message = msg.into();
        self.msg_kind = MsgKind::Error;
    }

    /// Construye la configuración a partir del formulario.
    pub fn form_config(&self) -> Result<ProxyConfig, String> {
        let mut pc = crate::proxy::parse_url(&self.url.text)?;
        pc.no_proxy = self.no_proxy.text.clone();
        Ok(pc)
    }

    /// Carga en el formulario la configuración presente en los archivos,
    /// priorizando la del archivo del shell (igual que la versión original).
    pub fn reload_form_from_files(&mut self) {
        if let Some(pc) = parse_config_from_file(&self.targets.term) {
            let source = display_path(&self.targets.term);
            self.set_form(&pc);
            self.info(format!("Configuración cargada desde {source}"));
        } else if let Some(pc) = parse_config_from_file(&self.targets.etc) {
            let source = display_path(&self.targets.etc);
            self.set_form(&pc);
            self.info(format!("Configuración cargada desde {source}"));
        } else {
            self.info(
                "No se encontró configuración de proxy; escribe la URL del proxy para configurarlo",
            );
        }
    }

    fn set_form(&mut self, pc: &ProxyConfig) {
        self.url.set_text(build_url(pc));
        self.no_proxy.set_text(pc.no_proxy.clone());
    }

    /// F3: recarga historial, estado y formulario desde los archivos.
    pub fn reload_from_disk(&mut self) {
        self.history = load_history();
        self.select_index(self.history.len().checked_sub(1));
        self.reload_form_from_files();
        self.enabled = is_proxy_enabled(&self.targets.files());
        if self.msg_kind == MsgKind::Info {
            self.info(format!("Recargado desde los archivos ({})", self.message));
        }
    }

    fn select_index(&mut self, index: Option<usize>) {
        if self.history.is_empty() {
            self.hist_state.select(None);
        } else {
            self.hist_state.select(index);
        }
    }

    pub fn selected_history(&self) -> Option<usize> {
        self.hist_state
            .selected()
            .filter(|i| *i < self.history.len())
    }

    /// Carga la entrada seleccionada del historial en el formulario.
    pub fn load_selected(&mut self) {
        let Some(index) = self.selected_history() else {
            return;
        };
        let entry = self.history[index].clone();
        self.set_form(&entry.to_proxy_config());
        self.info(format!("Configuración cargada: {}", entry.display_label()));
    }

    /// F2: guarda la configuración actual en el historial.
    pub fn save_config(&mut self) {
        let pc = match self.form_config() {
            Ok(pc) => pc,
            Err(err) => return self.error(err),
        };
        let entry = HistoryEntry::from_proxy_config(&pc);
        match add_history_entry(&entry) {
            Ok(()) => {
                self.history = load_history();
                self.select_index(self.history.len().checked_sub(1));
                self.success(format!("Configuración guardada: {}", entry.display_label()));
            }
            Err(err) => self.error(format!("Error guardando la configuración: {err}")),
        }
    }

    /// Borra la entrada seleccionada del historial.
    pub fn delete_selected(&mut self) {
        let Some(index) = self.selected_history() else {
            return;
        };
        let entry = self.history[index].clone();
        self.history.remove(index);
        match save_history(&self.history) {
            Ok(()) => {
                let next = if self.history.is_empty() {
                    None
                } else {
                    Some(index.min(self.history.len() - 1))
                };
                self.hist_state.select(next);
                self.success(format!("Entrada eliminada: {}", entry.display_label()));
            }
            Err(err) => {
                self.history = load_history();
                self.error(format!("Error eliminando la entrada: {err}"));
            }
        }
    }

    /// F5: habilita o deshabilita el proxy en los dos archivos.
    pub fn toggle_proxy(&mut self, terminal: &mut DefaultTerminal) {
        let pc = match self.form_config() {
            Ok(pc) => pc,
            Err(err) => return self.error(err),
        };

        let enable = !is_proxy_enabled(&self.targets.files());

        for file in self.targets.files() {
            let result = if file == self.targets.etc {
                self.apply_via_pkexec(&pc, &file, enable, terminal)
            } else {
                change_proxy_config(&pc, &file, enable)
                    .map_err(|e| format!("no se pudo escribir {}: {e}", display_path(&file)))
            };
            if let Err(err) = result {
                self.enabled = is_proxy_enabled(&self.targets.files());
                return self.error(err);
            }
        }

        self.enabled = is_proxy_enabled(&self.targets.files());
        if self.enabled {
            self.success("Proxy habilitado correctamente");
        } else {
            self.success("Proxy deshabilitado correctamente");
        }
    }

    /// Escribe /etc/environment con permisos de administrador usando pkexec
    /// y el modo auxiliar `-apply` del propio binario (igual que la versión Go).
    fn apply_via_pkexec(
        &mut self,
        pc: &ProxyConfig,
        file: &Path,
        enable: bool,
        terminal: &mut DefaultTerminal,
    ) -> Result<(), String> {
        let exe = std::env::current_exe()
            .map_err(|e| format!("no se pudo obtener la ruta del ejecutable: {e}"))?;

        tui::leave(terminal).map_err(|e| format!("error restaurando la terminal: {e}"))?;
        println!(
            "Se solicitan permisos de administrador para escribir {}...\n",
            file.display()
        );

        let status = Command::new("pkexec")
            .arg(exe)
            .arg("-apply")
            .arg(format!("-scheme={}", pc.scheme))
            .arg(format!("-user={}", pc.user))
            .arg(format!("-pass={}", pc.pass))
            .arg(format!("-url={}", pc.host))
            .arg(format!("-port={}", pc.port))
            .arg(format!("-noproxy={}", pc.no_proxy))
            .arg(format!("-file={}", file.display()))
            .arg(format!("-enable={enable}"))
            .status();

        tui::enter_existing(terminal).map_err(|e| format!("error reiniciando la terminal: {e}"))?;

        let status = status.map_err(|e| format!("no se pudo ejecutar pkexec: {e}"))?;
        if !status.success() {
            return Err("pkexec no pudo aplicar el cambio (¿autenticación cancelada?)".to_string());
        }
        Ok(())
    }

    /// Persiste el ancho del panel en el archivo de ajustes.
    pub fn persist_settings(&self) {
        let mut settings = self.settings;
        settings.split_offset = self.split;
        let _ = save_settings(&settings);
    }

    fn resize_split(&mut self, delta: f64) {
        self.split = (self.split + delta).clamp(0.15, 0.85);
        self.persist_settings();
    }

    pub fn focus_next(&mut self, direction: i32) {
        let order = [Focus::History, Focus::Url, Focus::NoProxy];
        let current = order.iter().position(|f| *f == self.focus).unwrap_or(0) as i32;
        let len = order.len() as i32;
        let next = (current + direction).rem_euclid(len) as usize;
        self.focus = order[next];
    }

    pub fn handle_paste(&mut self, text: &str) {
        let input = match self.focus {
            Focus::Url => &mut self.url,
            Focus::NoProxy => &mut self.no_proxy,
            Focus::History => return,
        };
        let cleaned: String = text.chars().filter(|c| !c.is_control()).collect();
        input.insert_str(&cleaned);
    }

    pub fn handle_key(&mut self, key: KeyEvent, terminal: &mut DefaultTerminal) -> io::Result<()> {
        use KeyCode::*;

        if self.show_help {
            self.show_help = false;
            return Ok(());
        }

        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl {
            match key.code {
                Char('q') | Char('c') => {
                    self.should_quit = true;
                    return Ok(());
                }
                Char('u') => {
                    match self.focus {
                        Focus::Url => self.url.clear(),
                        Focus::NoProxy => self.no_proxy.clear(),
                        Focus::History => {}
                    }
                    return Ok(());
                }
                Left => {
                    self.resize_split(-0.02);
                    return Ok(());
                }
                Right => {
                    self.resize_split(0.02);
                    return Ok(());
                }
                _ => {}
            }
        }

        match key.code {
            F(1) => {
                self.show_help = true;
                return Ok(());
            }
            F(2) => {
                self.save_config();
                return Ok(());
            }
            F(3) => {
                self.reload_from_disk();
                return Ok(());
            }
            F(5) => {
                self.toggle_proxy(terminal);
                return Ok(());
            }
            Tab => {
                self.focus_next(1);
                return Ok(());
            }
            BackTab => {
                self.focus_next(-1);
                return Ok(());
            }
            Esc => {
                if self.focus == Focus::History {
                    self.should_quit = true;
                } else {
                    self.focus = Focus::History;
                }
                return Ok(());
            }
            _ => {}
        }

        match self.focus {
            Focus::History => self.handle_history_key(key),
            Focus::Url => {
                if handle_input_key(key, &mut self.url) {
                    self.focus_next(1);
                }
            }
            Focus::NoProxy => {
                if handle_input_key(key, &mut self.no_proxy) {
                    self.focus_next(1);
                }
            }
        }

        Ok(())
    }

    fn handle_history_key(&mut self, key: KeyEvent) {
        let len = self.history.len();
        if len == 0 {
            return;
        }
        let current = self.hist_state.selected();
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                let next = match current {
                    Some(0) | None => 0,
                    Some(i) => i - 1,
                };
                self.hist_state.select(Some(next));
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let next = match current {
                    Some(i) if i + 1 < len => i + 1,
                    Some(i) => i,
                    None => 0,
                };
                self.hist_state.select(Some(next));
            }
            KeyCode::Home => self.hist_state.select(Some(0)),
            KeyCode::End => self.hist_state.select(Some(len - 1)),
            KeyCode::PageUp => {
                let next = current.unwrap_or(0).saturating_sub(10);
                self.hist_state.select(Some(next));
            }
            KeyCode::PageDown => {
                let next = (current.unwrap_or(0) + 10).min(len - 1);
                self.hist_state.select(Some(next));
            }
            KeyCode::Enter => self.load_selected(),
            KeyCode::Delete | KeyCode::Char('d') => self.delete_selected(),
            KeyCode::Char('?') => self.show_help = true,
            _ => {}
        }
    }
}

/// Edita un campo de texto. Devuelve true si se pulsó Enter.
fn handle_input_key(key: KeyEvent, input: &mut Input) -> bool {
    match key.code {
        KeyCode::Char(c) => input.insert(c),
        KeyCode::Backspace => input.backspace(),
        KeyCode::Delete => input.delete(),
        KeyCode::Left => input.move_left(),
        KeyCode::Right => input.move_right(),
        KeyCode::Home => input.move_home(),
        KeyCode::End => input.move_end(),
        KeyCode::Enter => return true,
        _ => {}
    }
    false
}

/// Muestra una ruta con `~` cuando está dentro del HOME.
pub fn display_path(path: &Path) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    if !home.is_empty()
        && let Ok(rest) = path.strip_prefix(&home)
    {
        if rest.as_os_str().is_empty() {
            return "~".to_string();
        }
        return format!("~/{}", rest.display());
    }
    path.display().to_string()
}
