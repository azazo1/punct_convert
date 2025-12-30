use std::{io::Write, thread, time::Duration};

use clap::Parser;
use clipboard_rs::{Clipboard, ClipboardContent, ContentFormat};
use html5ever::{parse_document, serialize, tendril::TendrilSink};
use mac_notification_sys::Notification;
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use tempfile::NamedTempFile;
use tracing::{info, warn};

const ICON_BYTES: &[u8] = include_bytes!("../res/clipboard.png");

#[derive(Parser)]
#[command(author, about, version, long_about = None)]
struct AppArgs {
    #[clap(short, long, help = "convert current clipboard and quit.")]
    oneshot: bool,
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

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let args = AppArgs::parse();
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
}
