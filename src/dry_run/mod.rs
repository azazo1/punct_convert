mod ansi_state;
mod render;
mod theme;

use clipboard_rs::{Clipboard, ClipboardContext, ContentFormat};

use self::{render::render_dry_run_entry_with_theme, theme::DryRunTheme};
use crate::convert::{convert_html_string, convert_str};

pub fn print_dry_run_report(ctx: &ClipboardContext) {
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
