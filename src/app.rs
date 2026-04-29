use std::{thread, time::Duration};

use clipboard_rs::{Clipboard, ClipboardContent, ContentFormat};
use tracing::{info, warn};

use crate::{
    args::AppArgs,
    convert::{convert_html_string, convert_str},
    dry_run::print_dry_run_report,
    notify::send_conversion_notification,
};

pub fn run(args: AppArgs) {
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

        if ctx.has(ContentFormat::Html)
            && handle_html_clipboard(&ctx, &mut last_text, &mut last_html)
        {
            continue;
        }

        handle_text_clipboard(&ctx, &mut last_text);
    }
}

fn handle_html_clipboard(
    ctx: &clipboard_rs::ClipboardContext,
    last_text: &mut Option<String>,
    last_html: &mut Option<String>,
) -> bool {
    let Ok(html) = ctx.get_html() else {
        return false;
    };
    if last_html
        .as_ref()
        .map(|h| h.as_str() == html.as_str())
        .unwrap_or(false)
    {
        return true;
    }

    info!("get clipboard(html)");

    let mut new_clipboard_content = Vec::new();

    if let Ok(text_plain) = ctx.get_text() {
        info!("get clipboard(text) at the same time.");
        if let Some(converted_text) = convert_str(text_plain.as_str()) {
            new_clipboard_content.push(ClipboardContent::Text(converted_text.clone()));
            *last_text = Some(converted_text);
            info!("clipboard text converted.");
        }
    }

    if let Some(converted_html) = convert_html_string(html.as_str()) {
        new_clipboard_content.push(ClipboardContent::Html(converted_html.clone()));
        *last_html = Some(converted_html);
        info!("clipboard html converted.");
    } else {
        *last_html = Some(html);
    }

    if new_clipboard_content.is_empty() {
        return true;
    }
    if let Err(e) = ctx.set(new_clipboard_content) {
        warn!("failed to set clipboard: {e}");
        return true;
    }

    send_conversion_notification(true);
    true
}

fn handle_text_clipboard(ctx: &clipboard_rs::ClipboardContext, last_text: &mut Option<String>) {
    let Ok(text) = ctx.get_text() else {
        return;
    };
    if last_text
        .as_ref()
        .map(|txt| txt.as_str() == text.as_str())
        .unwrap_or(false)
    {
        return;
    }

    info!("get clipboard:\n{}", text);

    *last_text = Some(text.clone());

    let Some(text) = convert_str(text.as_str()) else {
        info!("no chinese punct");
        return;
    };

    let Ok(()) = ctx.set_text(text.clone()) else {
        warn!("failed to set clipboard text");
        return;
    };

    send_conversion_notification(false);
}
