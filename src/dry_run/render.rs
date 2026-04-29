use arborium::{AnsiHighlighter, theme::builtin};
use markup_fmt::{Language, config::FormatOptions, format_text};

use crate::convert::{convert_html_string, convert_str};

use super::{ansi_state::AnsiSgrState, theme::DryRunTheme};

fn format_html_for_display(input: &str) -> String {
    format_text(
        input,
        Language::Html,
        &FormatOptions::default(),
        |code, _| Ok::<_, std::convert::Infallible>(code.into()),
    )
    .map(|formatted| formatted.trim_end().to_owned())
    .unwrap_or_else(|_| input.to_owned())
}

fn highlight_html_for_display(input: &str, theme: DryRunTheme) -> String {
    if !theme.enabled() {
        return input.to_owned();
    }

    let mut highlighter = AnsiHighlighter::new(builtin::catppuccin_mocha().clone());
    highlighter
        .highlight("html", input)
        .unwrap_or_else(|_| input.to_owned())
}

fn highlight_spaces_for_display(input: &str, theme: DryRunTheme) -> String {
    if !theme.enabled() {
        return input.to_owned();
    }

    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars();
    let mut at_line_start = true;
    let mut sgr_state = AnsiSgrState::default();
    let visible_space = theme.visible_space();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            output.push(ch);
            let mut sequence = String::new();
            for next in chars.by_ref() {
                output.push(next);
                sequence.push(next);
                if next == 'm' {
                    break;
                }
            }
            sgr_state.apply_escape_sequence(&sequence);
            continue;
        }

        if ch == '\n' {
            output.push(ch);
            at_line_start = true;
            continue;
        }

        if ch == ' ' {
            if at_line_start {
                output.push(ch);
            } else {
                output.push_str(&visible_space);
                if let Some(restore) = sgr_state.restore_sequence() {
                    output.push_str(&restore);
                }
            }
        } else {
            output.push(ch);
            at_line_start = false;
        }
    }
    output
}

fn format_dry_run_content(
    label: &str,
    content: &str,
    format_as_markup: bool,
    theme: DryRunTheme,
) -> String {
    let rendered = match (label, format_as_markup) {
        ("html", true) => highlight_html_for_display(&format_html_for_display(content), theme),
        ("html", false) if content == "<no changes>" => theme.placeholder(content),
        _ => content.to_owned(),
    };

    highlight_spaces_for_display(&rendered, theme)
}

fn append_report_block(
    output: &mut String,
    theme: DryRunTheme,
    label: &str,
    title: &str,
    content: &str,
    format_as_markup: bool,
) {
    let formatted = format_dry_run_content(label, content, format_as_markup, theme);
    output.push_str(&theme.block_header(title));
    output.push('\n');
    output.push_str(&formatted);
    if !formatted.ends_with('\n') {
        output.push('\n');
    }
    let end_title = format!(
        "--- end {} ---",
        title.trim_start_matches("--- ").trim_end_matches(" ---")
    );
    output.push_str(&theme.block_header(&end_title));
    output.push('\n');
}

pub(super) fn render_dry_run_entry_with_theme(
    label: &str,
    original: &str,
    converted: Option<&str>,
    theme: DryRunTheme,
) -> String {
    let mut output = String::new();
    output.push_str(&theme.section_header(&format!("=== {label} ===")));
    output.push('\n');
    output.push_str(&theme.summary("status: "));
    let status = if converted.is_some() {
        theme.status_will_convert("will convert")
    } else {
        theme.status_no_changes("no changes")
    };
    output.push_str(&status);
    output.push('\n');

    let original_title = format!("--- original {label} ---");
    append_report_block(&mut output, theme, label, &original_title, original, true);

    let converted_title = format!("--- converted {label} ---");
    match converted {
        Some(converted) => {
            append_report_block(&mut output, theme, label, &converted_title, converted, true)
        }
        None => append_report_block(
            &mut output,
            theme,
            label,
            &converted_title,
            "<no changes>",
            false,
        ),
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn render_dry_run_entry(label: &str, original: &str, converted: Option<&str>) -> String {
        render_dry_run_entry_with_theme(label, original, converted, DryRunTheme::plain())
    }

    #[test]
    fn test_render_dry_run_entry_for_html() {
        let out = render_dry_run_entry(
            "html",
            "<div class=container></div>",
            Some("<div class=container data-id=1></div>"),
        );
        assert!(out.contains("=== html ==="));
        assert!(out.contains("status: will convert"));
        assert!(out.contains("--- original html ---"));
        assert!(out.contains(r#"<div class="container"></div>"#));
        assert!(out.contains("--- converted html ---"));
        assert!(out.contains(r#"<div class="container" data-id="1"></div>"#));
        assert!(!out.contains("\x1b["));
    }

    #[test]
    fn test_render_dry_run_entry_without_changes() {
        let out = render_dry_run_entry("text", "plain ascii text", None);
        assert!(out.contains("=== text ==="));
        assert!(out.contains("status: no changes"));
        assert!(out.contains("<no changes>"));
    }

    #[test]
    fn test_format_html_for_display_uses_markup_formatter() {
        let input = "<div class=container></div>";
        let out = format_html_for_display(input);
        assert_eq!(out, r#"<div class="container"></div>"#);
    }

    #[test]
    fn test_format_html_for_display_falls_back_on_invalid_input() {
        let input = "<div>";
        let out = format_html_for_display(input);
        assert_eq!(out, input);
    }

    #[test]
    fn test_render_dry_run_entry_html_no_changes_keeps_placeholder() {
        let out = render_dry_run_entry("html", "<p>plain ascii</p>", None);
        assert!(out.contains("--- converted html ---\n<no changes>\n--- end converted html ---"));
    }

    #[test]
    fn test_render_dry_run_entry_with_theme_adds_ansi_colors() {
        let out = render_dry_run_entry_with_theme(
            "html",
            "<html><body><p>你好</p></body></html>",
            Some("<html><body><p>你好</p></body></html>"),
            DryRunTheme { enabled: true },
        );
        assert!(out.contains("\x1b["));
        assert!(out.contains("=== html ==="));
        assert!(out.contains("will convert"));
        assert!(out.contains("html"));
    }

    #[test]
    fn test_highlight_spaces_for_display_plain_theme_keeps_spaces() {
        let out = highlight_spaces_for_display("a b", DryRunTheme::plain());
        assert_eq!(out, "a b");
    }

    #[test]
    fn test_highlight_spaces_for_display_colored_theme_marks_spaces() {
        let out = highlight_spaces_for_display("a b", DryRunTheme { enabled: true });
        assert!(out.starts_with("a"));
        assert!(out.ends_with("b"));
        assert!(out.contains("\x1b["));
        assert_ne!(out, "a b");
    }

    #[test]
    fn test_highlight_spaces_for_display_skips_leading_spaces() {
        let out = highlight_spaces_for_display("  a b\n c d", DryRunTheme { enabled: true });
        assert!(out.starts_with("  a"));
        assert!(out.contains("\n c"));
        assert!(out.contains("\x1b["));
    }

    #[test]
    fn test_highlight_spaces_for_display_restores_simple_ansi_style() {
        let input = "\x1b[31mab cd\x1b[0m";
        let out = highlight_spaces_for_display(input, DryRunTheme { enabled: true });
        assert!(out.contains("\x1b[31mab"));
        assert!(out.contains("\x1b[31mcd\x1b[0m"));
    }

    #[test]
    fn test_highlight_spaces_for_display_restores_extended_ansi_style() {
        let input = "\x1b[1;38;2;1;2;3mfoo bar\x1b[0m";
        let out = highlight_spaces_for_display(input, DryRunTheme { enabled: true });
        assert!(out.contains("\x1b[1;38;2;1;2;3mfoo"));
        assert!(out.contains("\x1b[1;38;2;1;2;3mbar\x1b[0m"));
    }

    #[test]
    fn test_html_conversion_functions_are_reused_by_report() {
        let html = "<p>你好，世界！</p>";
        let converted = convert_html_string(html);
        let out = render_dry_run_entry("html", html, converted.as_deref());
        assert!(out.contains("你好, 世界!"));
    }

    #[test]
    fn test_text_conversion_functions_are_reused_by_report() {
        let text = "你好，世界！";
        let converted = convert_str(text);
        let out = render_dry_run_entry("text", text, converted.as_deref());
        assert!(out.contains("你好, 世界!"));
    }
}
