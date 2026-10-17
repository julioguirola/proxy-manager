use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, List, ListItem, Paragraph, Wrap};

use crate::app::{App, Focus, MsgKind, display_path};
use crate::input::Input;
use crate::proxy::build_url;

fn border_style(focused: bool) -> Style {
    if focused {
        Style::new().fg(Color::Cyan)
    } else {
        Style::new().fg(Color::DarkGray)
    }
}

fn dim() -> Style {
    // Gris claro (ANSI 7): un blanco atenuado, legible en fondos oscuros.
    Style::new().fg(Color::Gray)
}

/// Estilo para textos de ejemplo dentro de los campos (placeholder).
fn placeholder_style() -> Style {
    Style::new().fg(Color::Gray).add_modifier(Modifier::DIM)
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let [main, footer] = Layout::vertical([Constraint::Min(3), Constraint::Length(2)]).areas(area);

    let left_pct = (app.split * 100.0).round().clamp(15.0, 85.0) as u16;
    let [left, right] = Layout::horizontal([
        Constraint::Percentage(left_pct),
        Constraint::Percentage(100 - left_pct),
    ])
    .areas(main);

    draw_history(frame, app, left);
    draw_form(frame, app, right);
    draw_footer(frame, footer);

    if app.show_help {
        draw_help(frame, area);
    }
}

fn draw_history(frame: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::History;
    let block = Block::bordered()
        .title(" Configuraciones Guardadas ")
        .title_alignment(Alignment::Center)
        .border_style(border_style(focused));

    if app.history.is_empty() {
        let inner = block.inner(area);
        frame.render_widget(block, area);
        frame.render_widget(
            Paragraph::new("Sin configuraciones guardadas")
                .style(dim())
                .alignment(Alignment::Center),
            inner,
        );
        return;
    }

    let current_url = app.form_config().ok().map(|pc| build_url(&pc));
    let items: Vec<ListItem> = app
        .history
        .iter()
        .map(|entry| {
            let marker = if Some(entry.url_string()) == current_url {
                "● "
            } else {
                "  "
            };
            ListItem::new(Line::from(format!("{marker}{}", entry.display_label())))
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::new()
                .fg(Color::Cyan)
                .add_modifier(Modifier::REVERSED),
        )
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, &mut app.hist_state);
}

fn draw_form(frame: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus != Focus::History;
    let block = Block::bordered()
        .title(" Configuración de Proxy ")
        .title_alignment(Alignment::Center)
        .border_style(border_style(focused));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::vertical([
        Constraint::Length(1), // estado
        Constraint::Length(1),
        Constraint::Length(3), // proxy url
        Constraint::Length(1),
        Constraint::Length(3), // no proxy
        Constraint::Length(1),
        Constraint::Min(1),    // mensaje
        Constraint::Length(1), // archivos
    ])
    .split(inner);

    // Estado del proxy + pista de la tecla de acción.
    let [status_area, hint_area] =
        Layout::horizontal([Constraint::Min(10), Constraint::Length(22)]).areas(chunks[0]);
    let (status_text, status_style) = if app.enabled {
        (
            "● Proxy Activo",
            Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
        )
    } else {
        (
            "○ Proxy Desactivado",
            Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )
    };
    frame.render_widget(Paragraph::new(status_text).style(status_style), status_area);
    let hint = if app.enabled {
        "F5: deshabilitar"
    } else {
        "F5: habilitar"
    };
    frame.render_widget(
        Paragraph::new(hint)
            .style(dim())
            .alignment(Alignment::Right),
        hint_area,
    );

    draw_input(
        frame,
        chunks[2],
        "Proxy URL",
        &app.url,
        app.focus == Focus::Url,
        "http://usuario:contraseña@host:3128",
    );
    draw_input(
        frame,
        chunks[4],
        "No Proxy",
        &app.no_proxy,
        app.focus == Focus::NoProxy,
        "localhost,127.0.0.1,.local",
    );

    let msg_style = match app.msg_kind {
        MsgKind::Error => Style::new().fg(Color::Red),
        MsgKind::Success => Style::new().fg(Color::Green),
        MsgKind::Info => Style::new(),
    };
    frame.render_widget(
        Paragraph::new(app.message.clone())
            .style(msg_style)
            .wrap(Wrap { trim: true }),
        chunks[6],
    );

    let files = format!(
        "Se aplica a: {} y {}",
        display_path(&app.targets.term),
        display_path(&app.targets.etc)
    );
    frame.render_widget(Paragraph::new(files).style(dim()), chunks[7]);
}

fn draw_input(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    input: &Input,
    focused: bool,
    placeholder: &str,
) {
    let block = Block::bordered()
        .title(format!(" {label} "))
        .title_style(Style::new().add_modifier(Modifier::BOLD))
        .border_style(border_style(focused));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let width = inner.width as usize;
    let (text, style, cursor_col) = if input.is_empty() {
        (placeholder.to_string(), placeholder_style(), 0)
    } else {
        (input.text.clone(), Style::new(), input.cursor_col())
    };

    let offset = if cursor_col >= width {
        cursor_col - width + 1
    } else {
        0
    };
    let visible = visible_from(&text, offset);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(visible, style))),
        inner,
    );

    if focused {
        let x = inner.x + (cursor_col - offset).min(width - 1) as u16;
        frame.set_cursor_position(Position::new(x, inner.y));
    }
}

/// Devuelve el texto desde la celda `offset` (desplazamiento horizontal).
fn visible_from(text: &str, offset: usize) -> String {
    if offset == 0 {
        return text.to_string();
    }
    let mut acc = 0usize;
    for (idx, ch) in text.char_indices() {
        if acc >= offset {
            return text[idx..].to_string();
        }
        acc += Span::raw(ch.to_string()).width();
    }
    String::new()
}

fn draw_footer(frame: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from(
            "Tab Foco │ ↑↓/jk Mover │ Enter Cargar │ Del Borrar │ F2 Guardar │ F5 Activar/Desactivar proxy",
        ),
        Line::from("F3 Recargar │ Ctrl+←/→ Ancho │ F1/? Ayuda │ Ctrl+Q Salir"),
    ];
    frame.render_widget(Paragraph::new(lines).style(dim()), area);
}

fn draw_help(frame: &mut Frame, area: Rect) {
    let header = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let key = Style::new().fg(Color::Cyan);
    let line = |k: &'static str, desc: &'static str| {
        Line::from(vec![
            Span::raw("    "),
            Span::styled(format!("{k:<20}"), key),
            Span::raw(desc),
        ])
    };

    let help = vec![
        Line::from(""),
        Line::from(Span::styled("  Navegación", header)),
        line("Tab / Shift+Tab", "Cambiar el foco entre paneles y campos"),
        line("↑ ↓ / k j", "Mover la selección en el historial"),
        line("Enter", "Cargar la configuración seleccionada"),
        line("Del / d", "Borrar la configuración seleccionada"),
        Line::from(""),
        Line::from(Span::styled("  Acciones", header)),
        line("F2", "Guardar la configuración actual en el historial"),
        line("F3", "Recargar historial, estado y formulario"),
        line("F5", "Habilitar / deshabilitar el proxy"),
        line("Ctrl + ← / →", "Ajustar el ancho del panel izquierdo"),
        line("Ctrl+Q / Ctrl+C", "Salir"),
        Line::from(""),
        Line::from(Span::styled("  Formato", header)),
        Line::from("    Proxy URL: http://usuario:contraseña@host:puerto"),
        Line::from("    No Proxy:  lista separada por comas"),
        Line::from("               (ej: localhost,127.0.0.1,.local)"),
        Line::from(""),
        Line::from("  El proxy se aplica a los dos archivos a la vez: la"),
        Line::from("  configuración del shell y /etc/environment (con pkexec)."),
        Line::from(""),
        Line::from(Span::styled("  Pulsa cualquier tecla para cerrar", dim())),
    ];

    let w = 72.min(area.width);
    let h = (help.len() as u16 + 2).min(area.height);
    if w < 10 || h < 4 {
        return;
    }
    let popup = Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    };
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(help).block(
            Block::bordered()
                .title(" Ayuda ")
                .border_style(border_style(true)),
        ),
        popup,
    );
}

#[cfg(test)]
mod tests {
    use super::draw;
    use crate::app::App;
    use crate::proxy::{ProxyConfig, Targets, build_url};
    use crate::storage::{HistoryEntry, Settings};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn buffer_text(terminal: &Terminal<TestBackend>) -> String {
        let buffer = terminal.backend().buffer();
        let mut out = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                out.push_str(buffer[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn dibuja_panel_y_formulario() {
        let dir = tempfile::tempdir().unwrap();
        let targets = Targets::new(dir.path().join(".bashrc"));
        let mut app = App::new(targets, Settings::default());
        app.history = vec![HistoryEntry {
            user: "jguilora".to_string(),
            pass: "03090660960".to_string(),
            url: "10.12.0.205".to_string(),
            port: 3128,
            no_proxy: ".xetid.cu,localhost".to_string(),
            scheme: "http".to_string(),
            ..HistoryEntry::default()
        }];
        let pc = ProxyConfig {
            user: "jguilora".to_string(),
            pass: "03090660960".to_string(),
            host: "10.12.0.205".to_string(),
            port: 3128,
            no_proxy: ".xetid.cu,localhost".to_string(),
            ..ProxyConfig::default()
        };
        app.url.set_text(build_url(&pc));
        app.no_proxy.set_text(pc.no_proxy.clone());
        app.enabled = true;
        app.message = "Proxy habilitado correctamente".to_string();

        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();

        let text = buffer_text(&terminal);
        for want in [
            "Configuraciones Guardadas",
            "Configuración de Proxy",
            "● Proxy Activo",
            "Proxy URL",
            "No Proxy",
            "http://jguilora:03090660960@10.12.0.205:3128",
            "jguilora | 10.12.0.205:3128",
            "Proxy habilitado correctamente",
        ] {
            assert!(
                text.contains(want),
                "no se encontró {want:?} en la pantalla:\n{text}"
            );
        }
    }
}
