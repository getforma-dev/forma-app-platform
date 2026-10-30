//! forma-json-v1, the restricted canonical JSON of contract edition
//! impl-2026-10-28.1, unchanged in edition impl-2026-10-29.2
//! (`contracts/README.md`; `loads` and `canonical` in
//! `contracts/validate.py`). Deliberately not RFC 8785/JCS.
//!
//! Values are null, booleans, strings of Unicode scalar values, integers of
//! magnitude at most 2^53 - 1, arrays and objects whose keys match
//! `[a-z][a-z0-9_]*`. Canonical bytes sort keys by byte order, keep array
//! order, add no whitespace, write integers in decimal and strings as UTF-8
//! without normalization, escaping only `"`, `\` and controls (`\b \f \n \r \t`,
//! otherwise a lowercase four-digit escape).
use super::AdmissionError;
use serde_json::{Map, Number, Value};

/// Largest document, checked on raw bytes before parsing and on canonical output.
pub const MAX_DOCUMENT_BYTES: usize = 1_048_576;
/// Deepest value position; the root value is at depth 0.
pub const MAX_DEPTH: usize = 32;
/// Largest magnitude of a JSON integer, 2^53 - 1.
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

type Result<T> = std::result::Result<T, AdmissionError>;

/// Canonical forma-json-v1 bytes of an in-memory value. Refuses floats, unsafe
/// integers, invalid keys, depth beyond [`MAX_DEPTH`] and output beyond
/// [`MAX_DOCUMENT_BYTES`].
pub fn encode(value: &Value) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    write_value(value, 0, &mut out)?;
    if out.len() > MAX_DOCUMENT_BYTES {
        return Err(AdmissionError::TooLarge(out.len()));
    }
    Ok(out)
}

/// Strictly decodes a forma-json-v1 document: at most [`MAX_DOCUMENT_BYTES`]
/// checked before parsing, strict UTF-8, RFC 8259 syntax with surrounding
/// whitespace, and the value restrictions of [`encode`]. Duplicate keys,
/// fractions, exponents, non-finite numbers and unpaired surrogate escapes are
/// refused; `-0` is the integer 0, as in the Python reference.
pub fn decode(bytes: &[u8]) -> Result<Value> {
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(AdmissionError::TooLarge(bytes.len()));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|error| refused(error.valid_up_to(), "invalid UTF-8"))?;
    let mut parser = Parser { text, at: 0 };
    parser.whitespace();
    let value = parser.value(0)?;
    parser.whitespace();
    if parser.at != text.len() {
        return Err(parser.error("trailing data"));
    }
    Ok(value)
}

/// An object key admitted by forma-json-v1.
fn valid_key(key: &str) -> bool {
    key.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && key
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn refused(at: usize, rule: &str) -> AdmissionError {
    AdmissionError::Encoding(format!("{rule} at byte {at}"))
}

fn write_value(value: &Value, depth: usize, out: &mut Vec<u8>) -> Result<()> {
    if depth > MAX_DEPTH {
        return Err(AdmissionError::Encoding("JSON nesting exceeds 32".into()));
    }
    match value {
        Value::Null => out.extend_from_slice(b"null"),
        Value::Bool(true) => out.extend_from_slice(b"true"),
        Value::Bool(false) => out.extend_from_slice(b"false"),
        Value::Number(number) => write_integer(number, out)?,
        Value::String(text) => write_string(text, out),
        Value::Array(items) => {
            out.push(b'[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(b',');
                }
                write_value(item, depth + 1, out)?;
            }
            out.push(b']');
        }
        Value::Object(map) => {
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_unstable_by(|left, right| left.0.cmp(right.0));
            out.push(b'{');
            for (index, (key, child)) in entries.into_iter().enumerate() {
                if !valid_key(key) {
                    return Err(AdmissionError::Encoding(
                        "non-ASCII or invalid object key".into(),
                    ));
                }
                if index > 0 {
                    out.push(b',');
                }
                write_string(key, out);
                out.push(b':');
                write_value(child, depth + 1, out)?;
            }
            out.push(b'}');
        }
    }
    Ok(())
}

fn write_integer(number: &Number, out: &mut Vec<u8>) -> Result<()> {
    let safe = if let Some(value) = number.as_u64() {
        (value <= MAX_SAFE_INTEGER).then(|| value.to_string())
    } else if let Some(value) = number.as_i64() {
        (value.unsigned_abs() <= MAX_SAFE_INTEGER).then(|| value.to_string())
    } else {
        return Err(AdmissionError::Encoding(
            "floating-point number refused".into(),
        ));
    };
    let text = safe.ok_or_else(|| AdmissionError::Encoding("unsafe JSON integer".into()))?;
    out.extend_from_slice(text.as_bytes());
    Ok(())
}

fn write_string(text: &str, out: &mut Vec<u8>) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    out.push(b'"');
    for &byte in text.as_bytes() {
        match byte {
            b'"' => out.extend_from_slice(b"\\\""),
            b'\\' => out.extend_from_slice(b"\\\\"),
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\r' => out.extend_from_slice(b"\\r"),
            b'\t' => out.extend_from_slice(b"\\t"),
            0x08 => out.extend_from_slice(b"\\b"),
            0x0c => out.extend_from_slice(b"\\f"),
            0x00..=0x1f => {
                out.extend_from_slice(b"\\u00");
                out.push(HEX[usize::from(byte >> 4)]);
                out.push(HEX[usize::from(byte & 0x0f)]);
            }
            // Multi-byte UTF-8 sequences never contain ASCII bytes.
            _ => out.push(byte),
        }
    }
    out.push(b'"');
}

/// Recursive descent over validated UTF-8; `at` always rests on a character
/// boundary because it advances only past ASCII bytes or whole scalars.
struct Parser<'a> {
    text: &'a str,
    at: usize,
}

impl Parser<'_> {
    fn bytes(&self) -> &[u8] {
        self.text.as_bytes()
    }

    fn peek(&self) -> Option<u8> {
        self.bytes().get(self.at).copied()
    }

    fn error(&self, rule: &str) -> AdmissionError {
        refused(self.at, rule)
    }

    fn whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value> {
        if depth > MAX_DEPTH {
            return Err(self.error("JSON nesting exceeds 32"));
        }
        match self.peek() {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => self.string().map(Value::String),
            Some(b't') => self.literal(b"true", Value::Bool(true)),
            Some(b'f') => self.literal(b"false", Value::Bool(false)),
            Some(b'n') => self.literal(b"null", Value::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(b'N' | b'I') => Err(self.error("non-finite number refused")),
            _ => Err(self.error("expected a JSON value")),
        }
    }

    fn literal(&mut self, word: &[u8], value: Value) -> Result<Value> {
        if self.bytes()[self.at..].starts_with(word) {
            self.at += word.len();
            Ok(value)
        } else {
            Err(self.error("expected a JSON value"))
        }
    }

    fn number(&mut self) -> Result<Value> {
        let start = self.at;
        let negative = self.peek() == Some(b'-');
        if negative {
            self.at += 1;
        }
        let digits = self.at;
        match self.peek() {
            Some(b'0') => self.at += 1,
            Some(b'1'..=b'9') => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.at += 1;
                }
            }
            Some(b'I') => return Err(refused(start, "non-finite number refused")),
            _ => return Err(refused(start, "invalid number")),
        }
        match self.peek() {
            Some(b'.' | b'e' | b'E') => {
                return Err(refused(start, "floating-point number refused"))
            }
            Some(b'0'..=b'9') => return Err(refused(start, "invalid number")),
            _ => {}
        }
        // 2^53 - 1 has sixteen digits; anything longer is unsafe before parsing.
        let magnitude = &self.bytes()[digits..self.at];
        let value = if magnitude.len() > 16 {
            None
        } else {
            magnitude.iter().try_fold(0_u64, |total, digit| {
                total.checked_mul(10)?.checked_add(u64::from(digit - b'0'))
            })
        };
        match value {
            Some(value) if value <= MAX_SAFE_INTEGER => Ok(match (negative, value) {
                (true, 1..) => Value::from(-(value as i64)),
                _ => Value::from(value),
            }),
            _ => Err(refused(start, "unsafe JSON integer")),
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value> {
        self.at += 1;
        let mut items = Vec::new();
        self.whitespace();
        if self.peek() == Some(b']') {
            self.at += 1;
            return Ok(Value::Array(items));
        }
        loop {
            self.whitespace();
            items.push(self.value(depth + 1)?);
            self.whitespace();
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b']') => {
                    self.at += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(self.error("expected ',' or ']'")),
            }
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value> {
        self.at += 1;
        let mut map = Map::new();
        self.whitespace();
        if self.peek() == Some(b'}') {
            self.at += 1;
            return Ok(Value::Object(map));
        }
        loop {
            self.whitespace();
            if self.peek() != Some(b'"') {
                return Err(self.error("expected an object key"));
            }
            let key_at = self.at;
            let key = self.string()?;
            if !valid_key(&key) {
                return Err(refused(key_at, "non-ASCII or invalid object key"));
            }
            if map.contains_key(&key) {
                return Err(refused(key_at, "duplicate object key"));
            }
            self.whitespace();
            if self.peek() != Some(b':') {
                return Err(self.error("expected ':'"));
            }
            self.at += 1;
            self.whitespace();
            let value = self.value(depth + 1)?;
            map.insert(key, value);
            self.whitespace();
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b'}') => {
                    self.at += 1;
                    return Ok(Value::Object(map));
                }
                _ => return Err(self.error("expected ',' or '}'")),
            }
        }
    }

    fn string(&mut self) -> Result<String> {
        self.at += 1;
        let mut out = String::new();
        loop {
            let run = self.at;
            while let Some(byte) = self.peek() {
                if byte == b'"' || byte == b'\\' || byte < 0x20 {
                    break;
                }
                self.at += 1;
            }
            // The run ends before an ASCII byte, so it is whole UTF-8.
            out.push_str(
                self.text
                    .get(run..self.at)
                    .ok_or_else(|| refused(run, "invalid UTF-8"))?,
            );
            match self.peek() {
                Some(b'"') => {
                    self.at += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.at += 1;
                    out.push(self.escape()?);
                }
                Some(_) => return Err(self.error("control character in string")),
                None => return Err(self.error("unterminated string")),
            }
        }
    }

    fn escape(&mut self) -> Result<char> {
        let start = self.at - 1;
        let simple = match self.peek() {
            Some(b'"') => '"',
            Some(b'\\') => '\\',
            Some(b'/') => '/',
            Some(b'b') => '\u{8}',
            Some(b'f') => '\u{c}',
            Some(b'n') => '\n',
            Some(b'r') => '\r',
            Some(b't') => '\t',
            Some(b'u') => {
                self.at += 1;
                let unit = self.hex4()?;
                let scalar = match unit {
                    0xd800..=0xdbff if self.bytes()[self.at..].starts_with(b"\\u") => {
                        self.at += 2;
                        match self.hex4()? {
                            low @ 0xdc00..=0xdfff => {
                                0x10000 + ((unit - 0xd800) << 10) + (low - 0xdc00)
                            }
                            _ => return Err(refused(start, "unpaired Unicode surrogate")),
                        }
                    }
                    0xd800..=0xdfff => return Err(refused(start, "unpaired Unicode surrogate")),
                    other => other,
                };
                return char::from_u32(scalar)
                    .ok_or_else(|| refused(start, "unpaired Unicode surrogate"));
            }
            _ => return Err(refused(start, "invalid escape")),
        };
        self.at += 1;
        Ok(simple)
    }

    fn hex4(&mut self) -> Result<u32> {
        let digits = self
            .bytes()
            .get(self.at..self.at + 4)
            .ok_or_else(|| self.error("invalid unicode escape"))?;
        let mut unit = 0;
        for &digit in digits {
            let nibble = match digit {
                b'0'..=b'9' => digit - b'0',
                b'a'..=b'f' => digit - b'a' + 10,
                b'A'..=b'F' => digit - b'A' + 10,
                _ => return Err(self.error("invalid unicode escape")),
            };
            unit = unit * 16 + u32::from(nibble);
        }
        self.at += 4;
        Ok(unit)
    }
}
