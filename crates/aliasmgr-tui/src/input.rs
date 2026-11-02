//! Single-line text input used by search, rename, and editor fields.
//!
//! The cursor is a character index, not a byte offset, so CJK and other
//! multi-byte input edits correctly. Key handling here never consults the
//! global keymap: while a text field is active every printable character is
//! input, including letters that are shortcuts elsewhere (`q`, `s`, `d`, `/`).

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextInput {
    value: String,
    /// Character index in `0..=char_count`.
    cursor: usize,
}

impl TextInput {
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        let cursor = value.chars().count();
        Self { value, cursor }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    pub fn clear(&mut self) {
        self.value.clear();
        self.cursor = 0;
    }

    pub fn set_value(&mut self, value: impl Into<String>) {
        self.value = value.into();
        self.cursor = self.value.chars().count();
    }

    pub fn insert_char(&mut self, character: char) {
        let byte = self.byte_index(self.cursor);
        self.value.insert(byte, character);
        self.cursor += 1;
    }

    pub fn insert_str(&mut self, text: &str) {
        for character in text.chars() {
            self.insert_char(character);
        }
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let byte = self.byte_index(self.cursor - 1);
        self.value.remove(byte);
        self.cursor -= 1;
    }

    pub fn delete(&mut self) {
        if self.cursor >= self.value.chars().count() {
            return;
        }
        let byte = self.byte_index(self.cursor);
        self.value.remove(byte);
    }

    pub fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.value.chars().count());
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end(&mut self) {
        self.cursor = self.value.chars().count();
    }

    /// Splits the value at the cursor: `(before, after)`.
    pub fn split_at_cursor(&self) -> (String, String) {
        let byte = self.byte_index(self.cursor);
        (
            self.value[..byte].to_string(),
            self.value[byte..].to_string(),
        )
    }

    fn byte_index(&self, char_index: usize) -> usize {
        self.value
            .char_indices()
            .nth(char_index)
            .map(|(index, _)| index)
            .unwrap_or(self.value.len())
    }

    /// Applies an editing key. Returns true when the value changed.
    ///
    /// Only unmodified (or Shift-modified) printable characters insert text;
    /// Ctrl/Alt combinations are left for the caller so globals such as
    /// Ctrl+S remain available. Navigation returns false (no value change).
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        if !is_actionable(&key) {
            return false;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        match key.code {
            KeyCode::Char(character) if !ctrl && !alt => {
                self.insert_char(character);
                true
            }
            KeyCode::Backspace if !ctrl && !alt => {
                self.backspace();
                true
            }
            KeyCode::Delete if !ctrl && !alt => {
                self.delete();
                true
            }
            KeyCode::Left if !ctrl && !alt => {
                self.left();
                false
            }
            KeyCode::Right if !ctrl && !alt => {
                self.right();
                false
            }
            KeyCode::Home => {
                self.home();
                false
            }
            KeyCode::End => {
                self.end();
                false
            }
            _ => false,
        }
    }
}

/// Windows terminals may report key releases; only press/repeat act.
pub fn is_actionable(key: &KeyEvent) -> bool {
    matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
}

pub fn is_ctrl(key: &KeyEvent, character: char) -> bool {
    is_actionable(key)
        && key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char(value) if value.eq_ignore_ascii_case(&character))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn inserts_and_deletes_at_unicode_boundaries() {
        let mut input = TextInput::new("");
        input.insert_char('中');
        input.insert_char('a');
        input.insert_char('文');
        assert_eq!(input.value(), "中a文");
        assert_eq!(input.cursor(), 3);
        input.left();
        input.left();
        assert_eq!(input.cursor(), 1);
        input.backspace();
        assert_eq!(input.value(), "a文");
        input.end();
        input.delete();
        assert_eq!(input.value(), "a文");
        input.home();
        input.delete();
        assert_eq!(input.value(), "文");
    }

    #[test]
    fn cursor_navigation_clamps() {
        let mut input = TextInput::new("ab");
        input.left();
        input.left();
        input.left();
        assert_eq!(input.cursor(), 0);
        input.right();
        input.right();
        input.right();
        assert_eq!(input.cursor(), 2);
        input.home();
        assert_eq!(input.cursor(), 0);
        input.end();
        assert_eq!(input.cursor(), 2);
    }

    #[test]
    fn handle_key_inserts_printable_and_ignores_control_combinations() {
        let mut input = TextInput::new("");
        assert!(input.handle_key(key(KeyCode::Char('q'))));
        assert!(input.handle_key(key(KeyCode::Char('/'))));
        assert_eq!(input.value(), "q/");
        assert!(!input.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)));
        assert_eq!(input.value(), "q/");
        assert!(input.handle_key(key(KeyCode::Backspace)));
        assert_eq!(input.value(), "q");
    }

    #[test]
    fn key_release_is_ignored() {
        let mut press = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        press.kind = KeyEventKind::Release;
        let mut input = TextInput::new("");
        assert!(!input.handle_key(press));
        assert!(input.is_empty());
    }

    #[test]
    fn split_at_cursor_preserves_unicode() {
        let mut input = TextInput::new("中a文");
        input.left();
        assert_eq!(
            input.split_at_cursor(),
            ("中a".to_string(), "文".to_string())
        );
    }

    #[test]
    fn control_key_helpers_are_modifier_aware() {
        let ctrl_s = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL);
        let upper_s = KeyEvent::new(KeyCode::Char('S'), KeyModifiers::CONTROL);
        assert!(is_ctrl(&ctrl_s, 's'));
        assert!(is_ctrl(&upper_s, 's'));
        assert!(!is_ctrl(&key(KeyCode::Char('s')), 's'));
    }
}
