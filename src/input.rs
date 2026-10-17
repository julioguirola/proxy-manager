/// Estado simple de un campo de texto de una línea para la TUI.
#[derive(Clone, Debug, Default)]
pub struct Input {
    pub text: String,
    /// Posición del cursor en caracteres (no bytes).
    pub cursor: usize,
}

impl Input {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let cursor = text.chars().count();
        Self { text, cursor }
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        *self = Self::new(text);
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Índice en bytes del cursor, para cortar el String.
    fn byte_index(&self) -> usize {
        self.text
            .char_indices()
            .nth(self.cursor)
            .map(|(i, _)| i)
            .unwrap_or(self.text.len())
    }

    /// Ancho visible (en celdas) del texto a la izquierda del cursor.
    pub fn cursor_col(&self) -> usize {
        ratatui::text::Span::raw(&self.text[..self.byte_index()]).width()
    }

    pub fn insert(&mut self, c: char) {
        let idx = self.byte_index();
        self.text.insert(idx, c);
        self.cursor += 1;
    }

    pub fn insert_str(&mut self, s: &str) {
        let idx = self.byte_index();
        self.text.insert_str(idx, s);
        self.cursor += s.chars().count();
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let idx = self.byte_index();
        let prev = self.text[..idx].chars().last().unwrap();
        let start = idx - prev.len_utf8();
        self.text.replace_range(start..idx, "");
        self.cursor -= 1;
    }

    pub fn delete(&mut self) {
        let idx = self.byte_index();
        if idx >= self.text.len() {
            return;
        }
        let c = self.text[idx..].chars().next().unwrap();
        self.text.replace_range(idx..idx + c.len_utf8(), "");
    }

    pub fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_right(&mut self) {
        if self.cursor < self.text.chars().count() {
            self.cursor += 1;
        }
    }

    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor = self.text.chars().count();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edicion_basica() {
        let mut input = Input::new("abc");
        input.insert('d');
        assert_eq!(input.text, "abcd");
        input.move_left();
        input.backspace();
        assert_eq!(input.text, "abd");
        input.move_home();
        input.delete();
        assert_eq!(input.text, "bd");
        input.move_end();
        input.insert_str("é!");
        assert_eq!(input.text, "bdé!");
        assert_eq!(input.cursor, 4);
        input.move_home();
        input.delete();
        assert_eq!(input.text, "dé!");
    }

    #[test]
    fn cursor_col_con_unicode() {
        let mut input = Input::new("ñ");
        input.move_end();
        assert_eq!(input.cursor_col(), 1);
    }
}
