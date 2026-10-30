//! What an uploaded file is: its extension must name one of the accepted
//! types and its bytes must carry that type's magic (a PDF starts `%PDF-`, a
//! PNG, JPEG or WebP its signature, an XLSX the ZIP local header) or, for
//! CSV, TXT and MD, be UTF-8 text with no NUL byte that does not start with
//! a binary signature. Anything else is refused before it is stored.

/// The accepted kinds and their media types.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Kind {
    /// `pdf`, `image`, `csv`, `xlsx` or `text`.
    pub kind: &'static str,
    pub media_type: &'static str,
}

const BINARY_SIGNATURES: &[&[u8]] = &[
    b"%PDF-",
    b"\x89PNG\r\n\x1a\n",
    b"\xff\xd8\xff",
    b"RIFF",
    b"PK\x03\x04",
    b"GIF8",
    b"\x7fELF",
    b"MZ",
];

fn text_ok(bytes: &[u8]) -> bool {
    let body = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    !body.is_empty()
        && !body.contains(&0)
        && std::str::from_utf8(body).is_ok()
        && !BINARY_SIGNATURES.iter().any(|sig| body.starts_with(sig))
}

/// The file's extension, lowercase, or empty.
pub fn extension(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default()
}

/// The kind of `bytes` named `name`, or `None` (refused: unsupported type,
/// or bytes that are not what the name says).
pub fn sniff(name: &str, bytes: &[u8]) -> Option<Kind> {
    let kind = |kind, media_type| Some(Kind { kind, media_type });
    match extension(name).as_str() {
        "pdf" if bytes.starts_with(b"%PDF-") => kind("pdf", "application/pdf"),
        "png" if bytes.starts_with(b"\x89PNG\r\n\x1a\n") => kind("image", "image/png"),
        "jpg" | "jpeg" if bytes.starts_with(b"\xff\xd8\xff") => kind("image", "image/jpeg"),
        "webp" if bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" => {
            kind("image", "image/webp")
        }
        "xlsx" if bytes.starts_with(b"PK\x03\x04") => kind(
            "xlsx",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ),
        "csv" if text_ok(bytes) => kind("csv", "text/csv"),
        "txt" if text_ok(bytes) => kind("text", "text/plain"),
        "md" if text_ok(bytes) => kind("text", "text/markdown"),
        _ => None,
    }
}

/// The stored file name: the last path component, control characters,
/// slashes and backslashes removed, at most 200 characters; `None` when
/// nothing is left.
pub fn file_name(raw: &str) -> Option<String> {
    let last = raw.rsplit(['/', '\\']).next().unwrap_or(raw);
    let cleaned: String = last
        .chars()
        .filter(|c| !c.is_control() && *c != '"')
        .take(200)
        .collect();
    let cleaned = cleaned.trim().to_owned();
    (!cleaned.is_empty() && cleaned != "." && cleaned != "..").then_some(cleaned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_and_magic_must_agree() {
        assert_eq!(sniff("a.pdf", b"%PDF-1.7\n").unwrap().kind, "pdf");
        assert_eq!(
            sniff("a.PNG", b"\x89PNG\r\n\x1a\n....").unwrap().media_type,
            "image/png"
        );
        assert_eq!(sniff("a.jpeg", b"\xff\xd8\xff\xe0").unwrap().kind, "image");
        assert_eq!(
            sniff("a.webp", b"RIFF\x10\0\0\0WEBPVP8 ")
                .unwrap()
                .media_type,
            "image/webp"
        );
        assert_eq!(sniff("a.xlsx", b"PK\x03\x04rest").unwrap().kind, "xlsx");
        assert_eq!(
            sniff("q3.csv", b"month,revenue\n2026-07,1\n").unwrap().kind,
            "csv"
        );
        assert_eq!(sniff("q3.csv", b"\xef\xbb\xbfa,b\n").unwrap().kind, "csv");
        assert_eq!(
            sniff("notes.md", b"# Notes").unwrap().media_type,
            "text/markdown"
        );
        // The wrong magic bytes for the name.
        assert!(sniff("q3.csv", b"\x89PNG\r\n\x1a\nxxxx").is_none());
        assert!(sniff("q3.csv", b"%PDF-1.4").is_none());
        assert!(sniff("q3.csv", b"a,b\0c").is_none());
        assert!(sniff("q3.csv", b"\xff\xfe\x00a").is_none());
        assert!(sniff("a.pdf", b"not a pdf").is_none());
        assert!(sniff("a.xlsx", b"a,b").is_none());
        assert!(sniff("a.exe", b"MZ").is_none());
        assert!(sniff("a.svg", b"<svg/>").is_none());
        assert!(sniff("a.html", b"<html>").is_none());
        assert!(sniff("empty.csv", b"").is_none());
    }

    #[test]
    fn names_lose_paths_and_controls() {
        assert_eq!(file_name("C:\\tmp\\q3.csv").as_deref(), Some("q3.csv"));
        assert_eq!(file_name("../../etc/passwd").as_deref(), Some("passwd"));
        assert_eq!(file_name("a\u{0}b\".csv").as_deref(), Some("ab.csv"));
        assert_eq!(file_name(".."), None);
        assert_eq!(file_name("   "), None);
        assert_eq!(file_name(&"x".repeat(300)).unwrap().len(), 200);
    }
}
