//! The parser and admission run inside WASM. No WASI or other host interface.
use calamine::{DataRef, Reader, Xlsx};
use quick_xml::{events::Event, Reader as XmlReader};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read};

type Result<T> = std::result::Result<T, String>;
const MIB: usize = 1024 * 1024;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    profile: String,
    format: String,
    #[serde(default)]
    sheet: Option<String>,
    /// Lower-only limits make exact boundary/failure controls inexpensive.
    #[serde(default)]
    limits: BTreeMap<String, usize>,
}
#[derive(Clone)]
struct Limits {
    input: usize,
    expanded: usize,
    rows: usize,
    columns: usize,
    entries: usize,
    strings: usize,
    cells: usize,
    sheets: usize,
    text: usize,
    output: usize,
}
impl Limits {
    fn new(c: &Config) -> Result<Self> {
        let demo = match c.profile.as_str() {
            "demo" => true,
            "platform" => false,
            _ => return Err("profile_unknown".into()),
        };
        let mut l = Self {
            input: match c.format.as_str() {
                "csv" => {
                    if demo {
                        10 * MIB
                    } else {
                        100 * MIB
                    }
                }
                "xlsx" => {
                    if demo {
                        5 * MIB
                    } else {
                        10 * MIB
                    }
                }
                _ => return Err("format_unsupported".into()),
            },
            expanded: if demo { 50 * MIB } else { 100 * MIB },
            rows: if c.format == "xlsx" {
                10000
            } else if demo {
                100000
            } else {
                1000000
            },
            columns: 32,
            entries: 512,
            strings: 100000,
            cells: if demo { 320000 } else { 1000000 },
            sheets: 16,
            text: 8192,
            output: 128 * MIB,
        };
        for (k, v) in &c.limits {
            let target = match k.as_str() {
                "input" => &mut l.input,
                "expanded" => &mut l.expanded,
                "rows" => &mut l.rows,
                "columns" => &mut l.columns,
                "entries" => &mut l.entries,
                "strings" => &mut l.strings,
                "cells" => &mut l.cells,
                "sheets" => &mut l.sheets,
                "text" => &mut l.text,
                "output" => &mut l.output,
                _ => return Err("limit_unknown".into()),
            };
            if *v == 0 || *v > *target {
                return Err("limit_override_may_only_reduce".into());
            }
            *target = *v;
        }
        Ok(l)
    }
}
fn ensure(ok: bool, code: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(code.into())
    }
}
fn bounded_text(s: &str, l: &Limits) -> Result<()> {
    ensure(s.len() <= l.text && s.chars().count() <= 2000, "text_limit")
}
fn attr(e: &quick_xml::events::BytesStart<'_>, name: &str) -> Result<Option<String>> {
    for a in e.attributes() {
        let a = a.map_err(|_| "xml_attribute")?;
        if a.key.as_ref() == name {
            return a
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map(|v| Some(v.into_owned()))
                .map_err(|_| "xml_attribute".into());
        }
    }
    Ok(None)
}
fn number(s: &str) -> Result<usize> {
    s.parse::<usize>().map_err(|_| "invalid_integer".into())
}
fn coord(s: &str, l: &Limits) -> Result<(u32, u32)> {
    let split = s
        .bytes()
        .position(|b| b.is_ascii_digit())
        .ok_or("cell_coordinate")?;
    ensure(
        split > 0 && s[..split].bytes().all(|b| b.is_ascii_uppercase()),
        "cell_coordinate",
    )?;
    let mut col = 0usize;
    for b in s[..split].bytes() {
        col = col
            .checked_mul(26)
            .and_then(|n| n.checked_add((b - b'A' + 1) as usize))
            .ok_or("cell_coordinate")?;
    }
    let row = number(&s[split..])?;
    ensure(
        row > 0 && col > 0 && row <= l.rows && col <= l.columns,
        "dimension_limit",
    )?;
    Ok((row as u32, col as u32))
}
fn path_name(s: &str) -> Result<String> {
    ensure(
        !s.starts_with('/') && !s.contains('\\') && !s.contains('\0') && s.len() <= 512,
        "archive_path",
    )?;
    ensure(s.split('/').all(|p| p != ".." && p != "."), "archive_path")?;
    Ok(s.to_owned())
}
fn le16(b: &[u8], p: usize) -> Result<usize> {
    b.get(p..p + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]) as usize)
        .ok_or("archive_header".into())
}
fn le32(b: &[u8], p: usize) -> Result<usize> {
    b.get(p..p + 4)
        .map(|s| u32::from_le_bytes(s.try_into().unwrap()) as usize)
        .ok_or("archive_header".into())
}
/// Admit a deliberately strict ZIP subset before any dependency trusts metadata.
/// No offsets repaired, alternate footers, extra-field aliases, extraction or ZIP64.
fn archive_directory(b: &[u8], l: &Limits) -> Result<()> {
    ensure(b.len() >= 22, "archive_header")?;
    let end = b.len() - 22;
    ensure(
        b.get(end..end + 4) == Some(b"PK\x05\x06") && le16(b, end + 20)? == 0,
        "archive_footer",
    )?;
    ensure(
        !b[..end].windows(4).any(|w| w == b"PK\x05\x06"),
        "archive_ambiguous_footer",
    )?;
    ensure(
        le16(b, end + 4)? == 0 && le16(b, end + 6)? == 0,
        "archive_multidisk",
    )?;
    let count = le16(b, end + 10)?;
    ensure(
        count <= l.entries && count == le16(b, end + 8)?,
        "entry_limit",
    )?;
    let size = le32(b, end + 12)?;
    let central = le32(b, end + 16)?;
    let mut p = central;
    ensure(p.checked_add(size) == Some(end), "archive_directory")?;
    let mut names = BTreeSet::new();
    let mut declared = 0usize;
    let mut spans = Vec::new();
    for _ in 0..count {
        ensure(b.get(p..p + 4) == Some(b"PK\x01\x02"), "archive_directory")?;
        let flags = le16(b, p + 8)?;
        let method = le16(b, p + 10)?;
        // UTF-8 and deflate option bits only: encryption and data descriptors refused.
        ensure(
            flags & !0x0806 == 0 && (method == 0 || method == 8),
            "archive_method_or_encryption",
        )?;
        let mode = le32(b, p + 38)? >> 16;
        ensure(
            mode & 0o7000 == 0 && matches!(mode & 0o170000, 0 | 0o100000 | 0o040000),
            "archive_unsafe_mode",
        )?;
        let compressed = le32(b, p + 20)?;
        let expanded = le32(b, p + 24)?;
        ensure(
            compressed != u32::MAX as usize && expanded != u32::MAX as usize,
            "archive_zip64",
        )?;
        declared = declared.checked_add(expanded).ok_or("expanded_limit")?;
        ensure(declared <= l.expanded, "expanded_limit")?;
        let n = le16(b, p + 28)?;
        let x = le16(b, p + 30)?;
        let comment = le16(b, p + 32)?;
        ensure(x == 0 && comment == 0, "archive_extra_unsupported")?;
        ensure(le16(b, p + 34)? == 0, "archive_multidisk")?;
        let local = le32(b, p + 42)?;
        ensure(
            b.get(local..local + 4) == Some(b"PK\x03\x04"),
            "archive_local_header",
        )?;
        let raw = b.get(p + 46..p + 46 + n).ok_or("archive_header")?;
        let name = path_name(std::str::from_utf8(raw).map_err(|_| "archive_filename_utf8")?)?;
        ensure(
            names.insert(name.to_ascii_lowercase()),
            "archive_duplicate_path",
        )?;
        ensure(
            le16(b, local + 26)? == n && b.get(local + 30..local + 30 + n) == Some(raw),
            "archive_name_mismatch",
        )?;
        ensure(le16(b, local + 28)? == 0, "archive_extra_unsupported")?;
        ensure(
            le16(b, local + 6)? == flags
                && le16(b, local + 8)? == method
                && le32(b, local + 14)? == le32(b, p + 16)?
                && le32(b, local + 18)? == compressed
                && le32(b, local + 22)? == expanded,
            "archive_header_mismatch",
        )?;
        let data_end = local
            .checked_add(30 + n)
            .and_then(|v| v.checked_add(compressed))
            .ok_or("archive_header")?;
        ensure(data_end <= central, "archive_payload_bounds")?;
        spans.push((local, data_end));
        p = p.checked_add(46 + n).ok_or("archive_header")?;
        ensure(p <= end, "archive_header")?;
    }
    ensure(p == end, "archive_entry_count")?;
    spans.sort_unstable();
    let mut previous = 0;
    for (start, end) in spans {
        ensure(start == previous, "archive_overlap_or_gap")?;
        previous = end;
    }
    ensure(previous == central, "archive_overlap_or_gap")
}
#[derive(Default)]
struct Preflight {
    raw_numbers: BTreeMap<String, BTreeMap<(u32, u32), String>>,
    sheets: BTreeMap<String, String>,
    /// Sheet names in workbook order, for `sheet_selection_required`.
    sheet_order: Vec<String>,
    rels: BTreeMap<String, String>,
    expanded: usize,
    cells: usize,
    strings: usize,
}
fn scan_xml(name: &str, bytes: &[u8], l: &Limits, p: &mut Preflight) -> Result<()> {
    let xml = std::str::from_utf8(bytes).map_err(|_| "xml_utf8_required")?;
    let mut reader = XmlReader::from_str(xml);
    let mut depth = 0usize;
    let mut coords = BTreeSet::new();
    let mut current = None;
    let mut in_value = false;
    let mut cell_open = false;
    let mut value_seen = false;
    let mut value = String::new();
    let mut worksheet_cells = 0usize;
    let worksheet = name.starts_with("xl/worksheets/") && name.ends_with(".xml");
    loop {
        match reader.read_event().map_err(|_| "xml_invalid")? {
            Event::Start(e) | Event::Empty(e) => {
                let local = e.local_name();
                let tag = local.as_ref();
                ensure(e.len() <= l.text, "xml_token_limit")?;
                // Counting open elements is performed by checking the lexical self-closing terminator.
                let pos = reader.buffer_position() as usize;
                let self_closed = pos >= 2 && bytes.get(pos - 2..pos) == Some(b"/>");
                if !self_closed {
                    depth += 1;
                    ensure(depth <= 32, "xml_depth_limit")?;
                }
                if tag == "f" {
                    return Err("formula_refused".into());
                }
                if tag == "Relationship" {
                    ensure(
                        attr(&e, "TargetMode")?.as_deref() != Some("External"),
                        "external_link_refused",
                    )?;
                    if let Some(t) = attr(&e, "Type")? {
                        ensure(
                            !t.to_ascii_lowercase().contains("externallink")
                                && !t.to_ascii_lowercase().contains("vba"),
                            "external_link_or_macro_refused",
                        )?;
                    }
                    if name == "xl/_rels/workbook.xml.rels" {
                        let id = attr(&e, "Id")?.ok_or("relationship_id")?;
                        let target = attr(&e, "Target")?.ok_or("relationship_target")?;
                        let target = if target.starts_with("/xl/") {
                            target[1..].to_owned()
                        } else {
                            format!("xl/{target}")
                        };
                        path_name(&target)?;
                        ensure(
                            p.rels.insert(id, target).is_none(),
                            "relationship_duplicate",
                        )?;
                    }
                }
                if tag == "Override" || tag == "Default" {
                    if let Some(t) = attr(&e, "ContentType")? {
                        let t = t.to_ascii_lowercase();
                        ensure(!t.contains("macro") && !t.contains("vba"), "macro_refused")?;
                    }
                }
                if tag == "sheet" && name == "xl/workbook.xml" {
                    let title = attr(&e, "name")?.ok_or("sheet_name")?;
                    bounded_text(&title, l)?;
                    let id = attr(&e, "r:id")?.ok_or("sheet_relationship")?;
                    ensure(
                        p.sheets.insert(title.clone(), id).is_none(),
                        "sheet_duplicate",
                    )?;
                    p.sheet_order.push(title);
                    ensure(p.sheets.len() <= l.sheets, "sheet_limit")?;
                }
                if tag == "sst" {
                    for a in ["uniqueCount", "count"] {
                        if let Some(n) = attr(&e, a)? {
                            ensure(number(&n)? <= l.strings, "shared_string_limit")?;
                        }
                    }
                }
                if tag == "si" {
                    p.strings += 1;
                    ensure(p.strings <= l.strings, "shared_string_limit")?;
                }
                if worksheet && tag == "dimension" {
                    if let Some(v) = attr(&e, "ref")? {
                        for c in v.split(':') {
                            coord(c, l)?;
                        }
                    }
                }
                if worksheet && tag == "row" {
                    if let Some(v) = attr(&e, "r")? {
                        ensure(number(&v)? <= l.rows, "dimension_limit")?;
                    }
                }
                if worksheet && tag == "c" {
                    ensure(!cell_open, "cell_nested")?;
                    cell_open = !self_closed;
                    value_seen = false;
                    let c = coord(&attr(&e, "r")?.ok_or("cell_coordinate_required")?, l)?;
                    ensure(coords.insert(c), "cell_duplicate")?;
                    p.cells += 1;
                    worksheet_cells += 1;
                    ensure(
                        p.cells <= l.cells && worksheet_cells <= l.rows * l.columns,
                        "cell_limit",
                    )?;
                    let t = attr(&e, "t")?;
                    current = if t.as_deref().is_none() || t.as_deref() == Some("n") {
                        Some(c)
                    } else {
                        None
                    };
                }
                if worksheet && tag == "v" {
                    ensure(cell_open, "cell_value_structure")?;
                    ensure(!value_seen, "cell_value_duplicate")?;
                    value_seen = true;
                    in_value = !self_closed;
                    value.clear();
                }
            }
            Event::End(e) => {
                ensure(depth > 0, "xml_invalid")?;
                depth -= 1;
                if e.local_name().as_ref() == "v" && in_value {
                    if let Some(c) = current {
                        p.raw_numbers
                            .entry(name.to_owned())
                            .or_default()
                            .insert(c, value.clone());
                    }
                    in_value = false;
                }
                if e.local_name().as_ref() == "c" {
                    current = None;
                    cell_open = false;
                }
            }
            Event::Text(e) => {
                ensure(e.len() <= l.text, "xml_token_limit")?;
                if in_value {
                    value.push_str(e.as_ref());
                    ensure(value.len() <= l.text, "text_limit")?;
                }
            }
            Event::CData(e) => {
                ensure(e.len() <= l.text, "xml_token_limit")?;
                if in_value {
                    return Err("numeric_cdata_refused".into());
                }
            }
            Event::DocType(_) => return Err("doctype_refused".into()),
            Event::Decl(e) => {
                if let Some(enc) = e.encoding() {
                    ensure(
                        enc.map_err(|_| "xml_invalid")?
                            .eq_ignore_ascii_case("UTF-8"),
                        "xml_utf8_required",
                    )?;
                }
            }
            Event::GeneralRef(e) => {
                ensure(e.len() <= 32, "xml_entity_limit")?;
                if in_value {
                    return Err("numeric_entity_refused".into());
                }
            }
            Event::Eof => break,
            Event::Comment(e) => ensure(e.len() <= l.expanded, "xml_token_limit")?,
            Event::PI(_) => return Err("xml_processing_instruction_refused".into()),
        }
    }
    ensure(depth == 0, "xml_invalid")
}
fn preflight(bytes: &[u8], l: &Limits) -> Result<Preflight> {
    archive_directory(bytes, l)?;
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| "archive_invalid")?;
    let mut p = Preflight::default();
    ensure(zip.len() <= l.entries, "entry_limit")?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|_| "archive_entry")?;
        let name = entry.name().map_err(|_| "archive_filename")?.to_string();
        let lower = name.to_ascii_lowercase();
        ensure(
            !lower.contains("vbaproject") && !lower.ends_with(".bin"),
            "macro_or_binary_refused",
        )?;
        ensure(!lower.contains("externallinks/"), "external_link_refused")?;
        let remaining = l.expanded - p.expanded;
        let mut data = Vec::new();
        entry
            .by_ref()
            .take((remaining + 1) as u64)
            .read_to_end(&mut data)
            .map_err(|_| "archive_expansion_or_crc")?;
        p.expanded += data.len();
        ensure(p.expanded <= l.expanded, "expanded_limit")?;
        ensure(data.len() as u64 == entry.size(), "archive_size_mismatch")?;
        if lower.ends_with(".xml") || lower.ends_with(".rels") {
            scan_xml(&name, &data, l, &mut p)?;
        }
    }
    ensure(!p.sheets.is_empty(), "workbook_sheets_missing")?;
    Ok(p)
}
fn exact_number(raw: &str) -> Result<Value> {
    // Keep decimal coefficients exact; never round through calamine's f64.
    let (negative, body) = raw.strip_prefix('-').map_or((false, raw), |s| (true, s));
    let mut parts = body.split(['e', 'E']);
    let mantissa = parts.next().unwrap_or("");
    let exponent = parts
        .next()
        .map_or(Ok(0), |s| s.parse::<i32>().map_err(|_| "number_exponent"))?;
    ensure(
        parts.next().is_none() && (-100..=100).contains(&exponent),
        "number_exponent",
    )?;
    let mut dot = mantissa.split('.');
    let whole = dot.next().unwrap_or("");
    let frac = dot.next().unwrap_or("");
    ensure(
        dot.next().is_none()
            && !whole.is_empty()
            && whole
                .bytes()
                .chain(frac.bytes())
                .all(|b| b.is_ascii_digit()),
        "number_literal",
    )?;
    let mut coefficient = format!("{whole}{frac}").trim_start_matches('0').to_owned();
    if coefficient.is_empty() {
        coefficient.push('0');
    }
    let mut scale = frac.len() as i32 - exponent;
    if scale < 0 {
        // Zero is canonical even when an exponent moves the decimal point.
        // Appending zeros would turn 0e3/-0e3 into noncanonical "0000".
        if coefficient != "0" {
            ensure(
                coefficient.len() + (-scale) as usize <= 38,
                "number_precision",
            )?;
            coefficient.push_str(&"0".repeat((-scale) as usize));
        }
        scale = 0;
    }
    ensure(scale <= 18 && coefficient.len() <= 38, "number_precision")?;
    if negative && coefficient != "0" {
        coefficient.insert(0, '-');
    }
    if scale == 0 {
        Ok(json!({"type":"integer","value":coefficient,"unit":"unconfirmed"}))
    } else {
        Ok(json!({"type":"decimal","coefficient":coefficient,"scale":scale,"unit":"unconfirmed"}))
    }
}
fn push_cell(cells: &mut Vec<Value>, cell: Value, budget: &mut usize, l: &Limits) -> Result<()> {
    *budget = budget
        .checked_add(
            serde_json::to_vec(&cell)
                .map_err(|_| "output_encoding")?
                .len()
                + 1,
        )
        .ok_or("output_limit")?;
    ensure(*budget <= l.output, "output_limit")?;
    ensure(cells.len() < l.cells, "cell_limit")?;
    cells.push(cell);
    Ok(())
}
fn parse_xlsx(bytes: &[u8], c: &Config, l: &Limits) -> Result<Value> {
    let p = preflight(bytes, l)?;
    let selected = c.sheet.as_ref().ok_or("sheet_selection_required")?;
    let relationship = p.sheets.get(selected).ok_or("sheet_not_found")?;
    let path = p.rels.get(relationship).ok_or("sheet_relationship")?;
    let mut workbook = Xlsx::new(Cursor::new(bytes)).map_err(|_| "xlsx_invalid")?;
    let epoch_1904 = workbook.has_1904_epoch();
    let names = workbook.sheet_names().to_vec();
    let mut reader = workbook
        .worksheet_cells_reader(selected)
        .map_err(|_| "sheet_not_found")?;
    let mut cells = Vec::new();
    let mut budget = 1024usize;
    let mut rows = BTreeSet::new();
    while let Some(cell) = reader.next_cell().map_err(|_| "xlsx_cell_invalid")? {
        let row = cell.get_position().0 + 1;
        let column = cell.get_position().1 + 1;
        ensure(
            row as usize <= l.rows && column as usize <= l.columns,
            "dimension_limit",
        )?;
        rows.insert(row);
        let (value, raw, interpretation) = match cell.get_value() {
            DataRef::Empty => (json!({"type":"null"}), Value::Null, "literal"),
            DataRef::String(s) => {
                bounded_text(s, l)?;
                (json!({"type":"text","value":s}), json!(s), "literal")
            }
            DataRef::SharedString(s) => {
                bounded_text(s, l)?;
                (json!({"type":"text","value":s}), json!(s), "literal")
            }
            DataRef::Bool(b) => (json!({"type":"boolean","value":b}), json!(b), "literal"),
            DataRef::Int(_) | DataRef::Float(_) | DataRef::DateTime(_) => {
                let raw = p
                    .raw_numbers
                    .get(path)
                    .and_then(|m| m.get(&(row, column)))
                    .ok_or("number_original_missing")?;
                let date = matches!(cell.get_value(), DataRef::DateTime(_));
                (
                    exact_number(raw)?,
                    json!(raw),
                    if date {
                        "excel_serial_unconfirmed"
                    } else {
                        "unit_unconfirmed"
                    },
                )
            }
            DataRef::DateTimeIso(s) | DataRef::DurationIso(s) => {
                bounded_text(s, l)?;
                (
                    json!({"type":"text","value":s}),
                    json!(s),
                    "time_mapping_unconfirmed",
                )
            }
            DataRef::Error(_) => return Err("cell_error_refused".into()),
        };
        push_cell(
            &mut cells,
            json!({"coordinate":{"type":"xlsx","sheet":selected,"row":row,"column":column},"value":value,"original":raw,"interpretation":interpretation}),
            &mut budget,
            l,
        )?;
    }
    Ok(
        json!({"format":"xlsx","selected_sheet":selected,"available_sheets":names,"date_epoch_1904":epoch_1904,"rows":rows.len(),"cells":cells,"expanded_bytes":p.expanded,"formula_policy":"reject","mapping_status":"inspection_required"}),
    )
}
fn parse_csv(bytes: &[u8], l: &Limits) -> Result<Value> {
    std::str::from_utf8(bytes).map_err(|_| "csv_utf8_required")?;
    let mut reader = csv_core::Reader::new();
    let mut offset = 0usize;
    let mut field = vec![0u8; l.text];
    let mut used = 0usize;
    let mut row = 1usize;
    let mut col = 1usize;
    let mut physical_start = 1u64;
    let mut physical_line = 1u64;
    let mut previous_cr = false;
    let mut record_started = false;
    let mut cells = Vec::new();
    let mut record = Vec::new();
    let mut budget = 1024usize;
    loop {
        let (state, consumed, written) = reader.read_field(&bytes[offset..], &mut field[used..]);
        // csv-core reports CRLF record boundaries before consuming the LF. Track
        // physical lines independently, including bare CR and ignored blank lines.
        for &byte in &bytes[offset..offset + consumed] {
            if !record_started && byte != b'\r' && byte != b'\n' {
                physical_start = physical_line;
                record_started = true;
            }
            if byte == b'\r' || (byte == b'\n' && !previous_cr) {
                physical_line += 1;
            }
            previous_cr = byte == b'\r';
        }
        offset += consumed;
        used += written;
        match state {
            csv_core::ReadFieldResult::OutputFull => return Err("text_limit".into()),
            csv_core::ReadFieldResult::InputEmpty => continue,
            csv_core::ReadFieldResult::Field { record_end } => {
                ensure(row <= l.rows, "row_limit")?;
                ensure(col <= l.columns, "column_limit")?;
                let text = std::str::from_utf8(&field[..used]).map_err(|_| "csv_utf8_required")?;
                bounded_text(text, l)?;
                record.push((col, text.to_owned()));
                used = 0;
                col += 1;
                if record_end {
                    let ended_newline =
                        offset > 0 && matches!(bytes.get(offset - 1), Some(b'\n' | b'\r'));
                    let physical_end = if ended_newline {
                        physical_line.saturating_sub(1)
                    } else {
                        physical_line
                    };
                    for (column, text) in record.drain(..) {
                        push_cell(
                            &mut cells,
                            json!({"coordinate":{"type":"csv","logical_record":row,"physical_line_start":physical_start,"physical_line_end":physical_end.max(physical_start)},"column":column,"value":{"type":"text","value":text},"original":text,"interpretation":"mapping_unconfirmed"}),
                            &mut budget,
                            l,
                        )?;
                    }
                    row += 1;
                    col = 1;
                    physical_start = physical_line;
                    record_started = false;
                }
            }
            csv_core::ReadFieldResult::End => break,
        }
    }
    Ok(
        json!({"format":"csv","rows":row-1,"cells":cells,"formula_policy":"reject","mapping_status":"inspection_required","csv_dialect":"csv-core-0.1.13-permissive-utf8"}),
    )
}
pub fn parse_bytes(input: &[u8], config: &[u8]) -> Value {
    let run = || -> Result<Value> {
        ensure(config.len() <= 4096, "config_limit")?;
        let c: Config = serde_json::from_slice(config).map_err(|_| "config_invalid")?;
        let l = Limits::new(&c)?;
        ensure(input.len() <= l.input, "input_limit")?;
        let result = match c.format.as_str() {
            // Only a workbook that passes the whole-workbook preflight lists
            // its sheets; hostile content is refused for itself first.
            "xlsx" if c.sheet.is_none() => {
                let p = preflight(input, &l)?;
                return Ok(
                    json!({"status":"refused","reason":"sheet_selection_required","no_revision_created":true,"available_sheets":p.sheet_order}),
                );
            }
            "xlsx" => parse_xlsx(input, &c, &l)?,
            "csv" => {
                ensure(c.sheet.is_none(), "csv_has_no_sheet")?;
                parse_csv(input, &l)?
            }
            _ => return Err("format_unsupported".into()),
        };
        ensure(
            serde_json::to_vec(&result)
                .map_err(|_| "output_encoding")?
                .len()
                <= l.output,
            "output_limit",
        )?;
        Ok(
            json!({"status":"parsed","parser_version":1,"contract_edition":"impl-2026-10-28.1","profile":c.profile,"result":result}),
        )
    };
    match run() {
        Ok(value) => value,
        Err(reason) => json!({"status":"refused","reason":reason,"no_revision_created":true}),
    }
}
static mut OUTPUT: Vec<u8> = Vec::new();
#[no_mangle]
pub extern "C" fn alloc(len: u32) -> u32 {
    let mut bytes = vec![0u8; len as usize].into_boxed_slice();
    let p = bytes.as_mut_ptr();
    std::mem::forget(bytes);
    p as u32
}
/// Parse buffers provided through this module's allocator.
///
/// # Safety
/// Called only in the wasm32 instance. The trusted host must supply valid,
/// initialized, nonoverlapping allocations of the given lengths. No concurrent
/// calls may access this instance; the host drops its store after one parse.
#[no_mangle]
pub unsafe extern "C" fn parse(input: u32, len: u32, config: u32, config_len: u32) -> u32 {
    let value = parse_bytes(
        std::slice::from_raw_parts(input as *const u8, len as usize),
        std::slice::from_raw_parts(config as *const u8, config_len as usize),
    );
    let status = if value["status"] == "parsed" { 0 } else { 1 };
    OUTPUT = serde_json::to_vec(&value).unwrap();
    status
}
/// Read the output address after parse completes.
///
/// # Safety
/// wasm32 only; no concurrent mutation of OUTPUT is permitted. The host must
/// validate the complete output range against the instance's linear memory.
#[no_mangle]
pub unsafe extern "C" fn result_ptr() -> u32 {
    (&raw const OUTPUT).as_ref().unwrap().as_ptr() as u32
}
/// Read the output length after parse completes.
///
/// # Safety
/// wasm32 only; the host must not call concurrently with parse, and must bound
/// this length before copying or decoding the result.
#[no_mangle]
pub unsafe extern "C" fn result_len() -> u32 {
    (&raw const OUTPUT).as_ref().unwrap().len() as u32
}
