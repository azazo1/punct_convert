use std::io::Write;

use mac_notification_sys::Notification;
use tempfile::NamedTempFile;
use tracing::warn;

const ICON_BYTES: &[u8] = include_bytes!("../res/clipboard.png");

pub fn send_conversion_notification(preserve_format: bool) {
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

    let subtitle = if preserve_format {
        "中文符号已转换成英文符号，已保留格式"
    } else {
        "中文符号已转换成英文符号"
    };

    notification
        .title("成功转换标点符号")
        .subtitle(subtitle)
        .close_button("关闭")
        .send()
        .unwrap();
}
