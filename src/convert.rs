use html5ever::{parse_document, serialize, tendril::TendrilSink};
use markup5ever_rcdom::{Handle, NodeData, RcDom};

enum Convert {
    Converted(String),
    Raw(String),
}

fn convert_char(ch: char) -> Convert {
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
            rst.push(' ');
            prev_is_whitespace = true;
        }
    }
    rst
}

pub fn convert_str(input: &str) -> Option<String> {
    let convert_rst = input.chars().map(convert_char).reduce(|a, b| {
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

pub fn convert_html_string(input: &str) -> Option<String> {
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
