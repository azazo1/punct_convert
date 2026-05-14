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
    let text_plain = ctx.get_text().ok();
    if last_html.as_deref() == Some(html.as_str()) && last_text.as_deref() == text_plain.as_deref() {
        return true;
    }

    info!("get clipboard(html)");

    let converted_text = text_plain.as_deref().and_then(|text| {
        info!("get clipboard(text) at the same time.");
        let converted = convert_str(text);
        if converted.is_some() {
            info!("clipboard text converted.");
        }
        converted
    });
    let converted_html = convert_html_string(html.as_str());

    if converted_text.is_none() && converted_html.is_none() {
        *last_text = text_plain;
        *last_html = Some(html);
        return true;
    }

    let mut new_clipboard_content = Vec::new();

    if let Some(text) = converted_text.or(text_plain) {
        new_clipboard_content.push(ClipboardContent::Text(text.clone()));
        *last_text = Some(text);
    } else {
        *last_text = None;
    }

    if let Some(converted_html) = converted_html {
        new_clipboard_content.push(ClipboardContent::Html(converted_html.clone()));
        *last_html = Some(converted_html);
        info!("clipboard html converted.");
    } else {
        new_clipboard_content.push(ClipboardContent::Html(html.clone()));
        *last_html = Some(html);
    }

    append_passthrough_clipboard_content(ctx, &mut new_clipboard_content);

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

    let mut new_clipboard_content = vec![ClipboardContent::Text(text.clone())];
    append_passthrough_clipboard_content(ctx, &mut new_clipboard_content);

    let Ok(()) = ctx.set(new_clipboard_content) else {
        warn!("failed to set clipboard text");
        return;
    };

    *last_text = Some(text);

    send_conversion_notification(false);
}

fn append_passthrough_clipboard_content(
    ctx: &clipboard_rs::ClipboardContext,
    new_clipboard_content: &mut Vec<ClipboardContent>,
) {
    if ctx.has(ContentFormat::Rtf)
        && let Ok(rtf) = ctx.get_rich_text()
    {
        new_clipboard_content.push(ClipboardContent::Rtf(rtf));
    }

    if ctx.has(ContentFormat::Files)
        && let Ok(files) = ctx.get_files()
    {
        new_clipboard_content.push(ClipboardContent::Files(files));
    }
}
