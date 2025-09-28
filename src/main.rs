use std::{io::Write, thread, time::Duration};

use clipboard_rs::Clipboard;
use mac_notification_sys::Notification;
use tempfile::NamedTempFile;
use tracing::{info, warn};

const ICON_BYTES: &[u8] = include_bytes!("../res/clipboard.png");

enum Convert {
    Converted(String),
    Raw(String),
}

fn convert(ch: char) -> Convert {
    use Convert::*;
    match ch {
        '》' => Converted(">\0".into()),
        '《' => Converted("\0<".into()),
        '：' => Converted(":\0".into()),
        '；' => Converted(";\0".into()),
        '“' => Converted("\0\"".into()),
        '”' => Converted("\"\0".into()),
        '！' => Converted("!\0".into()),
        '…' => Converted("...".into()),
        '（' => Converted("\0(".into()),
        '）' => Converted(")\0".into()),
        '【' => Converted("\0[".into()),
        '】' => Converted("]\0".into()),
        '、' => Converted(",\0".into()),
        '。' => Converted(".\0".into()),
        '，' => Converted(",\0".into()),
        '？' => Converted("?\0".into()),
        _ => Raw(ch.into()),
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let mut last_text: Option<String> = None;
    loop {
        thread::sleep(Duration::from_secs_f32(0.5));
        let ctx = clipboard_rs::ClipboardContext::new().unwrap();
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

        let Some(convert_rst) = text.chars().map(convert).reduce(|a, b| {
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
        }) else {
            warn!("clipboard convert result empty");
            continue;
        };

        let mut text = match convert_rst {
            Convert::Converted(s) => s,
            Convert::Raw(_) => {
                info!("no chinese punct");
                continue;
            }
        };
        text = text
            .replace("\0\0", " ")
            .replace("\0\n", "\n")
            .trim_end_matches(|ch| ch == '\0')
            .replace("\0", " ");

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
