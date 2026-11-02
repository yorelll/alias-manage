//! Width-aware, control-character-safe text helpers.
//!
//! The TUI never trusts stored alias fields: values may contain tabs, escape
//! sequences, newlines, or double-width CJK characters. Everything rendered in
//! a single row goes through [`one_line`] and [`truncate_display`] so a crafted
//! value cannot break the layout or inject terminal control sequences.

use unicode_width::UnicodeWidthChar;

/// Collapses CR/LF/tab to a single space and sanitizes the rest.
pub fn one_line(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\r' | '\n' | '\t' => output.push(' '),
            character if character.is_control() => output.push('?'),
            character => output.push(character),
        }
    }
    output
}

/// The truncation marker used for the selected glyph mode.
pub fn ellipsis(ascii: bool) -> &'static str {
    if ascii {
        "..."
    } else {
        "\u{2026}"
    }
}

/// Display width in terminal cells, ignoring control characters.
pub fn display_width(value: &str) -> usize {
    value
        .chars()
        .map(|character| {
            if character.is_control() {
                0
            } else {
                UnicodeWidthChar::width(character).unwrap_or(0)
            }
        })
        .sum()
}

/// Truncates to at most `width` cells, appending an ellipsis when content was
/// removed. Wide characters are never split. The returned string is sanitized.
pub fn truncate_display(value: &str, width: usize, ascii: bool) -> String {
    let value = one_line(value);
    if width == 0 {
        return String::new();
    }
    if display_width(&value) <= width {
        return value;
    }
    let marker = ellipsis(ascii);
    let marker_width = display_width(marker);
    if width <= marker_width {
        // Not enough room for content plus a marker: show what fits.
        return take_width(&value, width);
    }
    let mut result = take_width(&value, width - marker_width);
    result.push_str(marker);
    result
}

/// Takes a prefix that occupies at most `width` cells.
pub fn take_width(value: &str, width: usize) -> String {
    let mut used = 0;
    let mut output = String::new();
    for character in value.chars() {
        let character_width = if character.is_control() {
            0
        } else {
            UnicodeWidthChar::width(character).unwrap_or(0)
        };
        if used + character_width > width {
            break;
        }
        used += character_width;
        output.push(character);
    }
    output
}

/// Pads with spaces to exactly `width` display cells, truncating if needed.
pub fn fit(value: &str, width: usize, ascii: bool) -> String {
    let mut value = truncate_display(value, width, ascii);
    let current = display_width(&value);
    if current < width {
        value.push_str(&" ".repeat(width - current));
    }
    value
}

/// Environment values are never rendered by default.
pub fn mask_value(value: &str) -> String {
    if value.is_empty() {
        "(empty)".to_string()
    } else {
        "***".to_string()
    }
}

/// Explicit masking for text that may embed an environment value. Used for
/// previews: every configured value is replaced with `***` before the shell
/// renderer sees it, so the generated text cannot leak it.
pub fn redact_environment<'a>(
    environment: impl IntoIterator<Item = (&'a String, &'a String)>,
) -> Vec<(String, String)> {
    environment
        .into_iter()
        .map(|(key, _value)| (key.clone(), "***".to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_line_replaces_control_characters_but_keeps_unicode() {
        assert_eq!(one_line("ok\u{1b}[31mred"), "ok?[31mred");
        assert_eq!(one_line("中文"), "中文");
        assert_eq!(one_line("a\nb\tc\r\nd"), "a b c  d");
        assert_eq!(one_line("x\u{7}y"), "x?y");
    }

    #[test]
    fn display_width_counts_cjk_as_two_cells() {
        assert_eq!(display_width("abc"), 3);
        assert_eq!(display_width("中文"), 4);
        assert_eq!(display_width("a中b"), 4);
        assert_eq!(display_width(""), 0);
    }

    #[test]
    fn truncation_never_splits_wide_characters() {
        // "中文abc" is 7 cells. With room for 5 and a one-cell marker the
        // result is "中文…" (2 + 2 + 1).
        assert_eq!(truncate_display("中文abc", 5, false), "中文\u{2026}");
        assert_eq!(truncate_display("中文abc", 5, true), "中...");
        // No content plus marker fits: take a clean wide-character prefix.
        assert_eq!(truncate_display("中文abc", 3, true), "中");
        assert_eq!(truncate_display("中文abc", 1, false), "");
    }

    #[test]
    fn truncation_keeps_short_values_and_sanitizes() {
        assert_eq!(truncate_display("abc", 10, false), "abc");
        assert_eq!(truncate_display("a\u{1b}b", 10, false), "a?b");
        assert_eq!(truncate_display("abcdef", 0, false), "");
    }

    #[test]
    fn fit_pads_and_truncates_to_exact_width() {
        assert_eq!(display_width(&fit("ab", 5, false)), 5);
        assert_eq!(fit("ab", 5, false), "ab   ");
        assert_eq!(display_width(&fit("中文中文", 5, false)), 5);
    }

    #[test]
    fn environment_values_are_masked() {
        assert_eq!(mask_value("secret"), "***");
        assert_eq!(mask_value(""), "(empty)");
        let environment = std::collections::BTreeMap::from([
            ("TOKEN".to_string(), "hunter2".to_string()),
            ("EMPTY".to_string(), String::new()),
        ]);
        let redacted: std::collections::BTreeMap<_, _> =
            redact_environment(environment.iter()).into_iter().collect();
        assert_eq!(redacted.get("TOKEN").map(String::as_str), Some("***"));
        assert_eq!(redacted.get("EMPTY").map(String::as_str), Some("***"));
        assert!(!format!("{redacted:?}").contains("hunter2"));
    }
}
