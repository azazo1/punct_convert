use std::{
    io::{IsTerminal, Write},
    thread,
    time::Duration,
};

use arborium::{AnsiHighlighter, theme::builtin};
use clap::Parser;
use clipboard_rs::{Clipboard, ClipboardContent, ContentFormat};
use colored::{ColoredString, Colorize};
use html5ever::{parse_document, serialize, tendril::TendrilSink};
use mac_notification_sys::Notification;
use markup_fmt::{Language, config::FormatOptions, format_text};
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use tempfile::NamedTempFile;
use tracing::{info, warn};

const ICON_BYTES: &[u8] = include_bytes!("../res/clipboard.png");

#[derive(Clone, Copy)]
struct DryRunTheme {
    enabled: bool,
}

impl DryRunTheme {
    #[allow(dead_code)]
    fn plain() -> Self {
        Self { enabled: false }
    }

    fn colored_stdout() -> Self {
        Self {
            enabled: std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none(),
        }
    }

    fn paint(self, text: &str, f: impl FnOnce(&str) -> ColoredString) -> String {
        if self.enabled {
            f(text).to_string()
        } else {
            text.to_owned()
        }
    }

    fn section_header(self, text: &str) -> String {
        self.paint(text, |s| s.bold().cyan())
    }

    fn block_header(self, text: &str) -> String {
        self.paint(text, |s| s.bold().blue())
    }

    fn summary(self, text: &str) -> String {
        self.paint(text, |s| s.bold().purple())
    }

    fn status_will_convert(self, text: &str) -> String {
        self.paint(text, |s| s.bold().green())
    }

    fn status_no_changes(self, text: &str) -> String {
        self.paint(text, |s| s.bold().yellow())
    }

    fn status_unreadable(self, text: &str) -> String {
        self.paint(text, |s| s.bold().red())
    }

    fn placeholder(self, text: &str) -> String {
        self.paint(text, |s| s.bright_black())
    }

    fn visible_space(self) -> String {
        self.paint(" ", |s| s.on_bright_black())
    }
}

#[derive(Parser)]
#[command(author, about, version, long_about = None)]
struct AppArgs {
    #[clap(short, long, help = "convert current clipboard and quit.")]
    oneshot: bool,
    #[clap(
        short = 'n',
        long,
        help = "inspect current clipboard and print planned conversions without modifying clipboard."
    )]
    dry_run: bool,
}

enum Convert {
    Converted(String),
    Raw(String),
}

fn convert(ch: char) -> Convert {
    use Convert::*;
    // 最后收集的时候, 两个及以上零字符则消除, 单个零字符变成空格.
    match ch {
        '》' => Converted("\0\0>\0".into()),
        '《' => Converted("\0<\0\0".into()),
        '：' => Converted("\0\0:\0".into()),
        '；' => Converted("\0\0;\0".into()),
        '“' => Converted("\0\"\0\0".into()),
        '”' => Converted("\0\0\"\0".into()),
        '！' => Converted("\0\0!\0".into()),
        '…' => Converted("\0\0...\0\0".into()),
        '（' => Converted("\0(\0\0".into()),
        '）' => Converted("\0\0)\0".into()),
        '【' => Converted("\0[\0\0".into()),
        '】' => Converted("\0\0]\0".into()),
        '、' => Converted("\0\0,\0".into()),
        '。' => Converted("\0\0.\0".into()),
        '，' => Converted("\0\0,\0".into()),
        '？' => Converted("\0\0?\0".into()),
        _ => Raw(ch.into()),
    }
}

fn merge_chars(s: &str) -> String {
    let chars: Vec<_> = s.chars().collect();
    let mut rst = String::with_capacity(s.len());
    let mut i = 0;
    let mut prev_is_whitespace = false;
    while i < chars.len() {
        let mut zero_cnt = 0;
        while i < chars.len() && chars[i] == '\0' {
            zero_cnt += 1;
            i += 1;
        }
        if zero_cnt == 0 {
            rst.push(chars[i]);
            prev_is_whitespace = chars[i].is_whitespace();
            i += 1;
        } else if zero_cnt == 1
            && !prev_is_whitespace
            && i < chars.len()
            && !chars[i].is_whitespace()
        {
            // 如果 \0 前面是空白符, 或者后面是空白符/末尾, 那么不添加空格.
            rst.push(' ');
            prev_is_whitespace = true;
        }
    }
    rst
}

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
    if !theme.enabled {
        return input.to_owned();
    }

    let mut highlighter = AnsiHighlighter::new(builtin::catppuccin_mocha().clone());
    highlighter
        .highlight("html", input)
        .unwrap_or_else(|_| input.to_owned())
}

fn highlight_spaces_for_display(input: &str, theme: DryRunTheme) -> String {
    if !theme.enabled {
        return input.to_owned();
    }

    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars();
    let mut at_line_start = true;
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            output.push(ch);
            for next in chars.by_ref() {
                output.push(next);
                if next == 'm' {
                    break;
                }
            }
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
                output.push_str(&theme.visible_space());
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

fn render_dry_run_entry_with_theme(
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

fn print_dry_run_report(ctx: &clipboard_rs::ClipboardContext) {
    let theme = DryRunTheme::colored_stdout();
    let has_html = ctx.has(ContentFormat::Html);
    let html = if has_html { ctx.get_html().ok() } else { None };
    let text = ctx.get_text().ok();

    let mut formats = Vec::new();
    if has_html {
        formats.push("html");
    }
    if text.is_some() {
        formats.push("text");
    }

    if formats.is_empty() {
        println!("{}", theme.summary("clipboard formats: <none>"));
        println!("no readable html/text clipboard content");
        return;
    }

    println!(
        "{}{}",
        theme.summary("clipboard formats: "),
        theme.section_header(&formats.join(", "))
    );

    if let Some(html) = html.as_deref() {
        println!();
        print!(
            "{}",
            render_dry_run_entry_with_theme(
                "html",
                html,
                convert_html_string(html).as_deref(),
                theme
            )
        );
    } else if has_html {
        println!();
        println!("{}", theme.section_header("=== html ==="));
        println!(
            "{}{}",
            theme.summary("status: "),
            theme.status_unreadable("unreadable")
        );
    }

    if let Some(text) = text.as_deref() {
        println!();
        print!(
            "{}",
            render_dry_run_entry_with_theme("text", text, convert_str(text).as_deref(), theme)
        );
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let args = AppArgs::parse();
    if args.dry_run {
        let ctx = clipboard_rs::ClipboardContext::new().unwrap();
        print_dry_run_report(&ctx);
        return;
    }

    let mut last_text: Option<String> = None;
    let mut last_html: Option<String> = None;
    let mut first_shot = true;

    for _ in std::iter::repeat_n((), 2).chain(std::iter::repeat_with(|| {
        thread::sleep(Duration::from_secs_f32(0.5));
    })) {
        if args.oneshot && !first_shot {
            break;
        }
        let ctx = clipboard_rs::ClipboardContext::new().unwrap();
        first_shot = false;

        if ctx.has(ContentFormat::Html) {
            let Ok(html) = ctx.get_html() else {
                continue;
            };
            if last_html
                .as_ref()
                .map(|h| h.as_str() == html.as_str())
                .unwrap_or(false)
            {
                continue;
            }

            info!("get clipboard(html)");

            let mut new_clipboard_content = Vec::new();

            if let Ok(text_plain) = ctx.get_text() {
                info!("get clipboard(text) at the same time.");
                if let Some(converted_text) = convert_str(text_plain.as_str()) {
                    new_clipboard_content.push(ClipboardContent::Text(converted_text.clone()));
                    last_text = Some(converted_text);
                    info!("clipboard text converted.");
                }
            }

            if let Some(converted_html) = convert_html_string(html.as_str()) {
                new_clipboard_content.push(ClipboardContent::Html(converted_html.clone()));
                last_html = Some(converted_html);
                info!("clipboard html converted.");
            } else {
                last_html = Some(html);
            };

            if new_clipboard_content.is_empty() {
                continue;
            }
            if let Err(e) = ctx.set(new_clipboard_content) {
                warn!("failed to set clipboard: {e}");
                continue;
            }

            let mut notification = Notification::new();
            let mut icon_file = NamedTempFile::new();
            match &mut icon_file {
                Ok(icon_file) => {
                    icon_file.write_all(ICON_BYTES).ok();
                    let icon_path = icon_file.path().to_str().unwrap_or("");
                    if !icon_path.is_empty() {
                        notification.app_icon(icon_path);
                    }
                }
                Err(e) => {
                    warn!("failed to create temp icon file: {}", e);
                }
            }
            notification
                .title("成功转换标点符号")
                .subtitle("中文符号已转换成英文符号，已保留格式")
                .close_button("关闭")
                .send()
                .unwrap();
            continue;
        }

        let Ok(text) = ctx.get_text() else {
            continue;
        };
        if last_text
            .as_ref()
            .map(|txt| txt.as_str() == text.as_str())
            .unwrap_or(false)
        {
            continue;
        }

        info!("get clipboard:\n{}", text);

        last_text = Some(text.clone());

        let Some(text) = convert_str(text.as_str()) else {
            info!("no chinese punct");
            continue;
        };

        let Ok(()) = ctx.set_text(text.clone()) else {
            warn!("failed to set clipboard text");
            continue;
        };

        let mut notification = Notification::new();
        let mut icon_file = NamedTempFile::new();
        match &mut icon_file {
            Ok(icon_file) => {
                icon_file.write_all(ICON_BYTES).ok();
                let icon_path = icon_file.path().to_str().unwrap_or("");
                if !icon_path.is_empty() {
                    notification.app_icon(icon_path);
                }
            }
            Err(e) => {
                warn!("failed to create temp icon file: {}", e);
            }
        }
        notification
            .title("成功转换标点符号")
            .subtitle("中文符号已转换成英文符号")
            .close_button("关闭")
            .send()
            .unwrap();
    }
}

fn convert_str(input: &str) -> Option<String> {
    let convert_rst = input.chars().map(convert).reduce(|a, b| {
        use Convert::*;
        match (a, b) {
            (Raw(mut a), Raw(b)) => {
                a.push_str(b.as_str());
                Raw(a)
            }
            (Converted(mut a), Raw(b)) => {
                a.push_str(b.as_str());
                Converted(a)
            }
            (Raw(mut a), Converted(b)) => {
                a.push_str(b.as_str());
                Converted(a)
            }
            (Converted(mut a), Converted(b)) => {
                a.push_str(b.as_str());
                Converted(a)
            }
        }
    })?;

    match convert_rst {
        Convert::Converted(mut s) => {
            s = merge_chars(&s);
            Some(s)
        }
        Convert::Raw(_) => None,
    }
}

fn convert_html_string(input: &str) -> Option<String> {
    let mut reader = input.as_bytes();
    let dom = parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut reader)
        .ok()?;

    fn walk(handle: Handle, changed: &mut bool) {
        let node = handle;
        if let NodeData::Text { contents } = &node.data {
            let mut borrow = contents.borrow_mut();
            if let Some(new_text) = convert_str(&borrow) {
                *borrow = new_text.into();
                *changed = true;
            }
        }
        let children = node.children.borrow().clone();
        for child in children {
            walk(child, changed);
        }
    }

    let mut changed = false;
    walk(dom.document.clone(), &mut changed);
    if !changed {
        return None;
    }
    let mut out = Vec::new();
    let handle = markup5ever_rcdom::SerializableHandle::from(dom.document.clone());
    serialize(&mut out, &handle, Default::default()).ok()?;
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn render_dry_run_entry(label: &str, original: &str, converted: Option<&str>) -> String {
        render_dry_run_entry_with_theme(label, original, converted, DryRunTheme::plain())
    }

    #[test]
    fn test_convert_str_basic() {
        let input = "你好，世界！“Rust” （2024）。“Rust”（2024）";
        let out = convert_str(input).expect("should convert");
        assert!(!out.contains('，'));
        assert!(!out.contains('！'));
        assert!(!out.contains('“'));
        assert!(!out.contains('”'));
        assert!(!out.contains('（'));
        assert!(!out.contains('）'));
        assert_eq!(out, r#"你好, 世界!"Rust" (2024)."Rust"(2024)"#);
    }

    #[test]
    fn test_convert_html_preserve_tags() {
        let input = r#"
<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <title>示例</title>
</head>
<body>
    <p>你好，<strong>“Rust”</strong>（2024）！</p>
</body>
</html>
        "#;
        let out = convert_html_string(input).expect("should convert html");
        assert!(out.contains("<strong>"));
        assert!(!out.contains('，'));
        assert!(!out.contains('！'));
        assert!(!out.contains('“'));
        assert!(!out.contains('”'));
        assert!(!out.contains('（'));
        assert!(!out.contains('）'));
        assert_eq!(
            out.trim(),
            r#"<!DOCTYPE html><html lang="zh-CN"><head>
    <meta charset="UTF-8">
    <title>示例</title>
</head>
<body>
    <p>你好,<strong> "Rust"</strong> (2024)!</p>


        </body></html>"#
        );
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
}
