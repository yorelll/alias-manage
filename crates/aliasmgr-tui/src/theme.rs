//! A deliberately small theme: one accent colour plus neutral, success,
//! warning and error. Status is always also expressed in text (`[on]`, `!`,
//! `[x]`), so a monochrome terminal loses no meaning.

use ratatui::{
    style::{Color, Modifier, Style},
    widgets::BorderType,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub color: bool,
    pub ascii: bool,
}

impl Theme {
    /// Detects the safe defaults for this terminal. `NO_COLOR` disables colour
    /// (the values are never required, only presence), `TERM=dumb` disables
    /// both colour and box drawing, and `ALIASMGR_TUI_ASCII=1` forces ASCII.
    pub fn detect() -> Self {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    pub fn from_lookup(mut lookup: impl FnMut(&str) -> Option<String>) -> Self {
        let no_color = lookup("NO_COLOR").is_some();
        let term = lookup("TERM").unwrap_or_default();
        let dumb = term.eq_ignore_ascii_case("dumb");
        let ascii = dumb || lookup("ALIASMGR_TUI_ASCII").as_deref() == Some("1");
        Self {
            color: !no_color && !dumb,
            ascii,
        }
    }

    pub fn color() -> Self {
        Self {
            color: true,
            ascii: false,
        }
    }

    pub fn mono() -> Self {
        Self {
            color: false,
            ascii: false,
        }
    }

    #[cfg(test)]
    pub fn ascii_only() -> Self {
        Self {
            color: false,
            ascii: true,
        }
    }

    fn colored(&self, color: Color, modifier: Modifier) -> Style {
        let style = Style::default().add_modifier(modifier);
        if self.color {
            style.fg(color)
        } else {
            style
        }
    }

    pub fn accent(&self) -> Style {
        self.colored(Color::Cyan, Modifier::empty())
    }

    pub fn title(&self) -> Style {
        self.colored(Color::Cyan, Modifier::BOLD)
    }

    pub fn selected(&self) -> Style {
        if self.color {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::REVERSED)
        }
    }

    pub fn focus(&self) -> Style {
        self.colored(Color::Cyan, Modifier::BOLD)
    }

    pub fn success(&self) -> Style {
        self.colored(Color::Green, Modifier::empty())
    }

    pub fn warning(&self) -> Style {
        self.colored(Color::Yellow, Modifier::empty())
    }

    pub fn error(&self) -> Style {
        self.colored(Color::Red, Modifier::BOLD)
    }

    pub fn muted(&self) -> Style {
        if self.color {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().add_modifier(Modifier::DIM)
        }
    }

    pub fn label(&self) -> Style {
        self.colored(Color::Cyan, Modifier::BOLD)
    }

    pub fn cursor(&self) -> Style {
        Style::default().add_modifier(Modifier::REVERSED)
    }

    pub fn borders(&self) -> BorderType {
        if self.ascii {
            BorderType::Plain
        } else {
            BorderType::Rounded
        }
    }

    pub fn ellipsis(&self) -> &'static str {
        crate::text::ellipsis(self.ascii)
    }

    /// Text marker for an enabled alias. Never colour-only.
    pub fn enabled_marker(&self, enabled: bool) -> &'static str {
        if enabled {
            "[on]"
        } else {
            "[off]"
        }
    }

    pub fn checkbox(&self, checked: bool) -> &'static str {
        if checked {
            "[x]"
        } else {
            "[ ]"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monochrome_theme_never_sets_a_colour() {
        let theme = Theme::mono();
        for style in [
            theme.accent(),
            theme.title(),
            theme.selected(),
            theme.focus(),
            theme.success(),
            theme.warning(),
            theme.error(),
            theme.muted(),
            theme.label(),
        ] {
            assert_eq!(style.fg, None);
            assert_eq!(style.bg, None);
        }
        // Selection remains visible without colour.
        assert!(theme.selected().add_modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn colour_theme_uses_one_accent_and_status_colours() {
        let theme = Theme::color();
        assert_eq!(theme.accent().fg, Some(Color::Cyan));
        assert_eq!(theme.title().fg, Some(Color::Cyan));
        assert_eq!(theme.success().fg, Some(Color::Green));
        assert_eq!(theme.warning().fg, Some(Color::Yellow));
        assert_eq!(theme.error().fg, Some(Color::Red));
    }

    #[test]
    fn detection_honours_no_color_dumb_and_ascii_override() {
        assert!(!Theme::from_lookup(|key| (key == "NO_COLOR").then(|| "1".to_string())).color);
        let dumb = Theme::from_lookup(|key| (key == "TERM").then(|| "dumb".to_string()));
        assert!(!dumb.color);
        assert!(dumb.ascii);
        assert!(
            Theme::from_lookup(|key| (key == "ALIASMGR_TUI_ASCII").then(|| "1".to_string())).ascii
        );
        let normal =
            Theme::from_lookup(|key| (key == "TERM").then(|| "xterm-256color".to_string()));
        assert!(normal.color);
        assert!(!normal.ascii);
    }

    #[test]
    fn ascii_theme_uses_plain_borders_and_markers() {
        let theme = Theme::ascii_only();
        assert_eq!(theme.borders(), BorderType::Plain);
        assert_eq!(theme.ellipsis(), "...");
        assert_eq!(theme.enabled_marker(true), "[on]");
        assert_eq!(theme.checkbox(false), "[ ]");
    }
}
