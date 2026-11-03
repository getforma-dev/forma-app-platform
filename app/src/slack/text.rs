//! What the bot says. Every text the bot posts is Slack `mrkdwn` built here:
//! every value that came from a record, a display name or a person is
//! escaped (`&`, `<`, `>`, Slack's three control characters) before it is
//! placed, so a merchant called `<!channel>` or `<https://evil|click>` is
//! shown as typed and never becomes a mention or a link (ADR-011 I5). The only
//! unescaped control sequences are the ones built here from server values: a
//! canvas link and a `<!date^…>` timestamp.
use serde_json::{json, Value};

/// Slack's escaping of the three control characters.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c if c.is_control() && c != '\n' => {}
            c => out.push(c),
        }
    }
    out
}
/// The inverse, for text Slack sent us (it escapes the same three).
pub fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// A server-built link: the URL is ours (an absolute https origin or a
/// same-origin path) and the label is escaped.
pub fn link(url: &str, label: &str) -> String {
    if url.starts_with("https://") {
        format!("<{url}|{}>", escape(label))
    } else {
        // No public origin (tests, local): plain text, never a control sequence.
        format!("{} ({})", escape(label), escape(url))
    }
}

/// A Slack date token rendered in the reader's own time zone, with a UTC
/// fallback for clients that cannot render it.
pub fn time(unix: u64) -> String {
    let stamp = crate::events::timestamp(unix);
    format!("<!date^{unix}^{{time}}|{} UTC>", &stamp[11..16])
}

pub fn section(text: &str) -> Value {
    json!({"type": "section", "text": {"type": "mrkdwn", "text": text}})
}
pub fn fields(pairs: &[(String, String)]) -> Value {
    json!({"type": "section", "fields": pairs.iter().take(10).map(|(label, value)| json!({
        "type": "mrkdwn", "text": format!("*{}*\n{}", escape(label), escape(value)),
    })).collect::<Vec<_>>()})
}
pub fn context(text: &str) -> Value {
    json!({"type": "context", "elements": [{"type": "mrkdwn", "text": text}]})
}
/// A button; `value` is an opaque server id (a draft id or a record id) that
/// the click handler judges again.
pub fn button(text: &str, action: &str, value: &str, style: Option<&str>) -> Value {
    let mut button = json!({
        "type": "button",
        "text": {"type": "plain_text", "text": text, "emoji": false},
        "action_id": action,
        "value": value,
    });
    if let Some(style) = style {
        button["style"] = Value::String(style.to_owned());
    }
    button
}
pub fn actions(block_id: &str, buttons: Vec<Value>) -> Value {
    json!({"type": "actions", "block_id": block_id, "elements": buttons})
}

/// An ephemeral reply (the slash command's own answer or a response_url
/// message), optionally replacing the message it answers.
pub fn ephemeral(text: &str, blocks: Option<Vec<Value>>, replace: bool) -> Value {
    let mut message = json!({"response_type": "ephemeral", "text": text});
    if let Some(blocks) = blocks {
        message["blocks"] = Value::Array(blocks);
    }
    if replace {
        message["replace_original"] = Value::Bool(true);
    } else {
        message["replace_original"] = Value::Bool(false);
    }
    message
}
