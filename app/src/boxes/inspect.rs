//! A CSV/XLSX in a box, read through the sealed parser (ADR-009 D9.1: one
//! `parser-host --job` process per parse, no network), as a header and text
//! rows; each column's type inferred from its values (APP-018 lite: the
//! member confirms or adjusts them); and, once confirmed, the canonical CSV
//! the normalizer reads (header = the field ids, every cell in the
//! normalization table's text form: money with exactly two places, dates
//! `YYYY-MM-DD`, timestamps `YYYY-MM-DDTHH:MM:SSZ`, booleans `true`/`false`).
//! A cell that does not fit its confirmed type refuses the whole file and
//! names the column; nothing is coerced silently.
use crate::imports::sandbox::{
    self, ExpectedInput, Format, ParseConfig, ParseRefusal, ParserIdentity,
};
use serde_json::{json, Map, Value};
use std::time::Duration;

/// A parse's deadline (well inside the guest's 120 s; demo files are small).
pub const PARSE_DEADLINE: Duration = Duration::from_secs(30);
/// Columns a box data file may have (the contract's field bound).
pub const MAX_COLUMNS: usize = 32;

/// The parsed table: the first record is the header.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Table {
    pub format: &'static str,
    pub sheet: Option<String>,
    pub header: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

/// A column type the member may confirm.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColumnType {
    Text,
    Integer,
    Decimal(u32),
    Money,
    Date,
    Timestamp,
    Boolean,
}
impl ColumnType {
    pub fn name(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Integer => "integer",
            Self::Decimal(_) => "decimal",
            Self::Money => "money",
            Self::Date => "date",
            Self::Timestamp => "timestamp",
            Self::Boolean => "boolean",
        }
    }
    pub fn parse(name: &str, scale: Option<u64>) -> Option<Self> {
        Some(match name {
            "text" => Self::Text,
            "integer" => Self::Integer,
            "decimal" => Self::Decimal(u32::try_from(scale.unwrap_or(2)).ok().filter(|s| *s <= 6)?),
            "money" => Self::Money,
            "date" => Self::Date,
            "timestamp" => Self::Timestamp,
            "boolean" => Self::Boolean,
            _ => return None,
        })
    }
    /// The contract value type (`library_sources.py`'s table).
    pub fn value_type(self) -> Value {
        match self {
            Self::Text => json!({"type": "text"}),
            Self::Integer => json!({"type": "integer", "unit": "count"}),
            Self::Decimal(scale) => json!({"type": "decimal", "unit": "number", "scale": scale}),
            Self::Money => json!({"type": "money", "currency": "USD", "minor_unit_exponent": 2}),
            Self::Date => json!({"type": "date"}),
            Self::Timestamp => json!({"type": "timestamp"}),
            Self::Boolean => json!({"type": "boolean"}),
        }
    }
    pub fn numeric(self) -> bool {
        matches!(self, Self::Integer | Self::Decimal(_) | Self::Money)
    }
}

/// One column: its field id, the header text, its type, nullability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Column {
    pub id: String,
    pub header: String,
    pub kind: ColumnType,
    pub nullable: bool,
}
impl Column {
    pub fn to_json(&self) -> Value {
        json!({"id": self.id, "header": self.header, "type": self.kind.name(), "nullable": self.nullable,
            "scale": match self.kind { ColumnType::Decimal(s) => json!(s), _ => Value::Null }})
    }
    /// The contract field.
    pub fn field(&self) -> Value {
        json!({"id": self.id, "value_type": self.kind.value_type(), "nullable": self.nullable})
    }
}

/// A parse that ended without a table; the code is neutral (the parser's own
/// refusal reason, or a fixed failure code).
pub fn refusal_code(refusal: &ParseRefusal) -> String {
    match refusal {
        ParseRefusal::Refused { reason, .. } => reason.clone(),
        ParseRefusal::Deadline => "parse_deadline".into(),
        ParseRefusal::ResourceLimit => "parse_resource_limit".into(),
        ParseRefusal::Crashed => "parse_failed".into(),
        ParseRefusal::Failure(code) => (*code).into(),
    }
}

async fn run(
    identity: &ParserIdentity,
    bytes: &[u8],
    format: Format,
    sheet: Option<String>,
) -> Result<Value, ParseRefusal> {
    let expected = ExpectedInput {
        sha256: super::store::sha256_hex(bytes),
        bytes: bytes.len() as u64,
    };
    let config = ParseConfig { format, sheet };
    sandbox::parse_with(
        identity,
        std::io::Cursor::new(bytes.to_vec()),
        &expected,
        &config,
        PARSE_DEADLINE,
    )
    .await
    .map(|output| output.document)
}

/// Parses a CSV or XLSX (the first sheet) into a table.
pub async fn parse(identity: &ParserIdentity, bytes: &[u8], kind: &str) -> Result<Table, String> {
    let (document, format, sheet) = match kind {
        "csv" => (
            run(identity, bytes, Format::Csv, None)
                .await
                .map_err(|r| refusal_code(&r))?,
            "csv",
            None,
        ),
        "xlsx" => {
            let sheets = match run(identity, bytes, Format::Xlsx, None).await {
                Err(ParseRefusal::Refused {
                    available_sheets: Some(sheets),
                    ..
                }) => sheets,
                Err(refusal) => return Err(refusal_code(&refusal)),
                Ok(_) => return Err("sheet_selection_required".into()),
            };
            let first = sheets.first().cloned().ok_or("no_sheet")?;
            (
                run(identity, bytes, Format::Xlsx, Some(first.clone()))
                    .await
                    .map_err(|r| refusal_code(&r))?,
                "xlsx",
                Some(first),
            )
        }
        _ => return Err("not_data".into()),
    };
    table_of(&document, format, sheet).ok_or_else(|| "parser_output_invalid".into())
}

/// Original-cell viewer, independent of inference/normalization. A fixed
/// read-only profile emits one row/column window, never the entire workbook.
/// Rows are one-based source coordinates, including the first/header row.
pub async fn original_page(
    identity: &ParserIdentity,
    bytes: &[u8],
    kind: &str,
    sheet_index: usize,
    offset: usize,
    column_offset: usize,
) -> Result<Value, String> {
    let window = sandbox::ViewWindow {
        row_offset: offset,
        column_offset,
    };
    if !window.valid() || sheet_index >= 16 {
        return Err("page_unavailable".into());
    }
    let run = |format, sheet| async move {
        let expected = ExpectedInput {
            sha256: super::store::sha256_hex(bytes),
            bytes: bytes.len() as u64,
        };
        sandbox::parse_view_with(
            identity,
            std::io::Cursor::new(bytes.to_vec()),
            &expected,
            &ParseConfig { format, sheet },
            window,
            PARSE_DEADLINE,
        )
        .await
        .map(|output| output.document)
    };
    let (document, sheets) = match kind {
        "csv" if sheet_index == 0 => (
            run(Format::Csv, None).await.map_err(|r| refusal_code(&r))?,
            Vec::new(),
        ),
        "xlsx" => {
            let sheets = match run(Format::Xlsx, None).await {
                Err(ParseRefusal::Refused {
                    reason,
                    available_sheets: Some(sheets),
                }) if reason == "sheet_selection_required"
                    && !sheets.is_empty()
                    && sheets.len() <= 16 =>
                {
                    sheets
                }
                Err(refusal) => return Err(refusal_code(&refusal)),
                Ok(_) => return Err("parser_output_invalid".into()),
            };
            let sheet = sheets.get(sheet_index).ok_or("sheet_not_found")?.clone();
            (
                run(Format::Xlsx, Some(sheet))
                    .await
                    .map_err(|r| refusal_code(&r))?,
                sheets,
            )
        }
        _ => return Err("not_data".into()),
    };
    original_page_of(&document, kind, sheets, sheet_index, offset, column_offset)
        .ok_or_else(|| "page_unavailable".into())
}

pub const ORIGINAL_PAGE_ROWS: usize = 50;
pub const ORIGINAL_PAGE_COLUMNS: usize = 32;
pub const ORIGINAL_MAX_ROWS: usize = 1_048_576;
pub const ORIGINAL_MAX_COLUMNS: usize = 16_384;
const ORIGINAL_MAX_CELL_BYTES: usize = 256 * 1024;
const ORIGINAL_MAX_CELL_CHARS: usize = 65_536;
const ORIGINAL_MAX_CELLS: usize = 4_000_000;
const ORIGINAL_PAGE_BYTES: usize = 8 * 1024 * 1024;

/// Preserve lexical values and sparse coordinates. Verify global metadata and
/// every emitted coordinate against the requested page before filling blanks.
fn original_page_of(
    document: &Value,
    kind: &str,
    sheets: Vec<String>,
    sheet_index: usize,
    offset: usize,
    column_offset: usize,
) -> Option<Value> {
    let window = sandbox::ViewWindow {
        row_offset: offset,
        column_offset,
    };
    if !window.valid() || document.get("status")? != "parsed" || document.get("profile")? != "view"
    {
        return None;
    }
    let result = document.get("result")?;
    if result.get("format")? != kind
        || result.get("row_offset")? != offset
        || result.get("column_offset")? != column_offset
        || result.get("page_rows")? != ORIGINAL_PAGE_ROWS
        || result.get("page_columns")? != ORIGINAL_PAGE_COLUMNS
    {
        return None;
    }
    if kind == "xlsx" {
        if sheets.len() > 16
            || sheets
                .iter()
                .any(|name| name.is_empty() || name.len() > 8192)
            || sheets
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != sheets.len()
            || result.get("selected_sheet")?.as_str()? != sheets.get(sheet_index)?
        {
            return None;
        }
    } else if kind != "csv" || !sheets.is_empty() || sheet_index != 0 {
        return None;
    }
    let row_count = usize::try_from(result.get("row_count")?.as_u64()?).ok()?;
    let column_count = usize::try_from(result.get("column_count")?.as_u64()?).ok()?;
    if row_count > ORIGINAL_MAX_ROWS
        || column_count > ORIGINAL_MAX_COLUMNS
        || (offset != 0 && offset >= row_count)
        || (column_offset != 0 && column_offset >= column_count)
        || ((row_count == 0) != (column_count == 0))
    {
        return None;
    }
    let row_end = offset.checked_add(ORIGINAL_PAGE_ROWS)?.min(row_count);
    let column_end = column_offset
        .checked_add(ORIGINAL_PAGE_COLUMNS)?
        .min(column_count);
    let contains = |row: usize, column: usize| {
        row > offset && row <= row_end && column > column_offset && column <= column_end
    };
    let cells = result.get("cells")?.as_array()?;
    if cells.len() > ORIGINAL_PAGE_ROWS * ORIGINAL_PAGE_COLUMNS {
        return None;
    }
    let mut page = std::collections::BTreeMap::new();
    let mut bytes = 0usize;
    for cell in cells {
        let coordinate = cell.get("coordinate")?;
        if coordinate.get("type")?.as_str()? != kind {
            return None;
        }
        let (row, column) = if kind == "csv" {
            (
                coordinate.get("logical_record")?.as_u64()?,
                cell.get("column")?.as_u64()?,
            )
        } else {
            if coordinate.get("sheet")?.as_str()? != sheets.get(sheet_index)? {
                return None;
            }
            (
                coordinate.get("row")?.as_u64()?,
                coordinate.get("column")?.as_u64()?,
            )
        };
        let (row, column) = (usize::try_from(row).ok()?, usize::try_from(column).ok()?);
        if !contains(row, column) {
            return None;
        }
        let value = match cell.get("original")? {
            Value::String(value)
                if value.len() <= ORIGINAL_MAX_CELL_BYTES
                    && value.chars().count() <= ORIGINAL_MAX_CELL_CHARS =>
            {
                value.clone()
            }
            Value::Bool(value) => value.to_string(),
            Value::Null => String::new(),
            _ => return None,
        };
        bytes = bytes.checked_add(value.len())?;
        if bytes > ORIGINAL_PAGE_BYTES || page.insert((row, column), value).is_some() {
            return None;
        }
    }
    let formula_count = usize::try_from(result.get("formula_count")?.as_u64()?).ok()?;
    let missing_formula_results =
        usize::try_from(result.get("missing_formula_results")?.as_u64()?).ok()?;
    if formula_count > ORIGINAL_MAX_CELLS
        || formula_count > row_count.checked_mul(column_count)?
        || missing_formula_results > formula_count
        || (kind == "csv" && formula_count != 0)
    {
        return None;
    }
    let formulas = result.get("formula_cells")?.as_array()?;
    if formulas.len() > formula_count || formulas.len() > ORIGINAL_PAGE_ROWS * ORIGINAL_PAGE_COLUMNS
    {
        return None;
    }
    let mut formula_cells = Vec::new();
    let mut coordinates = std::collections::BTreeSet::new();
    let mut missing_on_page = 0usize;
    for formula in formulas {
        let row = usize::try_from(formula.get("row")?.as_u64()?).ok()?;
        let column = usize::try_from(formula.get("column")?.as_u64()?).ok()?;
        let cached = formula.get("cached")?.as_bool()?;
        if !contains(row, column)
            || !coordinates.insert((row, column))
            || !page.contains_key(&(row, column))
        {
            return None;
        }
        if !cached {
            missing_on_page += 1;
            if page.get(&(row, column))?.as_str() != "" {
                return None;
            }
        }
        formula_cells.push(json!({"row":row,"column":column,"cached":cached}));
    }
    if missing_on_page > missing_formula_results {
        return None;
    }
    let rows: Vec<Vec<String>> = (offset + 1..=row_end)
        .map(|row| {
            (column_offset + 1..=column_end)
                .map(|column| page.remove(&(row, column)).unwrap_or_default())
                .collect()
        })
        .collect();
    Some(
        json!({"format":kind,"sheets":sheets,"sheet_index":sheet_index,
        "row_count":row_count,"column_count":column_count,"offset":offset,"column_offset":column_offset,
        "page_size":ORIGINAL_PAGE_ROWS,"column_page_size":ORIGINAL_PAGE_COLUMNS,"rows":rows,
        "formula_count":formula_count,"missing_formula_results":missing_formula_results,"formula_cells":formula_cells}),
    )
}

/// A typed cell value of the parser as display text.
fn cell_text(value: &Value) -> Option<String> {
    Some(match value.get("type")?.as_str()? {
        "null" => String::new(),
        "text" => value.get("value")?.as_str()?.to_owned(),
        "boolean" => value.get("value")?.as_bool()?.to_string(),
        "integer" => value.get("value")?.as_str()?.to_owned(),
        "decimal" => {
            let coefficient = value.get("coefficient")?.as_str()?;
            let scale = usize::try_from(value.get("scale")?.as_u64()?).ok()?;
            let (sign, digits) = match coefficient.strip_prefix('-') {
                Some(rest) => ("-", rest),
                None => ("", coefficient),
            };
            let padded = format!("{digits:0>width$}", width = scale + 1);
            let (whole, fraction) = padded.split_at(padded.len() - scale);
            format!("{sign}{whole}.{fraction}")
        }
        _ => return None,
    })
}

/// The parser document's cells as a header and rows (trailing empty rows dropped).
pub fn table_of(document: &Value, format: &'static str, sheet: Option<String>) -> Option<Table> {
    let cells = document.get("result")?.get("cells")?.as_array()?;
    let mut grid: std::collections::BTreeMap<u64, Vec<(u64, String)>> = Default::default();
    let mut width = 0u64;
    for cell in cells {
        let coordinate = cell.get("coordinate")?;
        let (row, column) = match coordinate.get("type")?.as_str()? {
            "csv" => (
                coordinate.get("logical_record")?.as_u64()?,
                cell.get("column")?.as_u64()?,
            ),
            "xlsx" => (
                coordinate.get("row")?.as_u64()?,
                coordinate.get("column")?.as_u64()?,
            ),
            _ => return None,
        };
        width = width.max(column);
        grid.entry(row)
            .or_default()
            .push((column, cell_text(cell.get("value")?)?));
    }
    if width == 0 || width as usize > MAX_COLUMNS {
        return None;
    }
    let mut rows: Vec<Vec<String>> = Vec::new();
    let first = *grid.keys().next()?;
    let last = *grid.keys().last()?;
    for row in first..=last {
        let mut values = vec![String::new(); width as usize];
        for (column, text) in grid.remove(&row).unwrap_or_default() {
            values[(column - 1) as usize] = text;
        }
        rows.push(values);
    }
    while rows
        .last()
        .is_some_and(|r| r.iter().all(|v| v.trim().is_empty()))
    {
        rows.pop();
    }
    let header = rows.first()?.iter().map(|h| h.trim().to_owned()).collect();
    let rows = rows.into_iter().skip(1).collect();
    Some(Table {
        format,
        sheet,
        header,
        rows,
    })
}

/// A lowercase local id (`[a-z][a-z0-9_]{0,63}`) from free text.
pub fn local_id(text: &str, fallback: &str) -> String {
    let mut out = String::new();
    for c in text.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
    }
    let mut out = out.trim_end_matches('_').to_owned();
    if out.is_empty() {
        out = fallback.to_owned();
    }
    if !out.starts_with(|c: char| c.is_ascii_lowercase()) {
        out = format!("c_{out}");
    }
    out.truncate(48);
    out.trim_end_matches('_').to_owned()
}

fn unique_ids(header: &[String]) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    header
        .iter()
        .enumerate()
        .map(|(index, text)| {
            let base = local_id(text, &format!("column_{}", index + 1));
            let mut id = base.clone();
            let mut n = 2;
            while !seen.insert(id.clone()) {
                id = format!("{base}_{n}");
                n += 1;
            }
            id
        })
        .collect()
}

const MONEY_WORDS: &[&str] = &[
    "revenue", "amount", "cost", "price", "spend", "budget", "arr", "mrr", "usd", "sales",
    "income", "expense", "profit", "salary", "payment", "$",
];

fn money_hint(header: &str) -> bool {
    let lower = header.to_lowercase();
    MONEY_WORDS.iter().any(|word| {
        if *word == "$" {
            lower.contains('$')
        } else {
            lower
                .split(|c: char| !c.is_ascii_alphanumeric())
                .any(|part| part == *word)
        }
    })
}

/// A number's text without a leading `$`, `+` and thousands commas, if it
/// is one; `(sign, whole, fraction)`.
fn number(text: &str) -> Option<(bool, String, String)> {
    let mut t = text.trim();
    let mut negative = false;
    if let Some(rest) = t.strip_prefix('-') {
        negative = true;
        t = rest;
    } else if let Some(rest) = t.strip_prefix('+') {
        t = rest;
    }
    let t = t.strip_prefix('$').unwrap_or(t);
    let t = if !negative {
        match t.strip_prefix('-') {
            Some(rest) => {
                negative = true;
                rest
            }
            None => t,
        }
    } else {
        t
    };
    let (whole, fraction) = match t.split_once('.') {
        Some((w, f)) => (w, f),
        None => (t, ""),
    };
    let whole_plain: String = if whole.contains(',') {
        let groups: Vec<&str> = whole.split(',').collect();
        let ok = !groups[0].is_empty()
            && groups[0].len() <= 3
            && groups[1..].iter().all(|g| g.len() == 3);
        if !ok {
            return None;
        }
        groups.concat()
    } else {
        whole.to_owned()
    };
    if whole_plain.is_empty()
        || !whole_plain.bytes().all(|b| b.is_ascii_digit())
        || !fraction.bytes().all(|b| b.is_ascii_digit())
        || (text.contains('.') && fraction.is_empty())
        || whole_plain.len() > 18
    {
        return None;
    }
    let whole_plain = whole_plain.trim_start_matches('0');
    let whole_plain = if whole_plain.is_empty() {
        "0"
    } else {
        whole_plain
    };
    Some((negative, whole_plain.to_owned(), fraction.to_owned()))
}

fn is_date(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
        && {
            let month: u32 = text[5..7].parse().unwrap_or(0);
            let day: u32 = text[8..10].parse().unwrap_or(0);
            (1..=12).contains(&month) && (1..=31).contains(&day)
        }
}

/// A timestamp in one of the accepted forms, as `YYYY-MM-DDTHH:MM:SSZ`.
fn timestamp(text: &str) -> Option<String> {
    let t = text.trim();
    if is_date(t) {
        return Some(format!("{t}T00:00:00Z"));
    }
    let t = t.strip_suffix('Z').unwrap_or(t);
    if t.len() != 19 || !is_date(&t[..10]) || !matches!(t.as_bytes()[10], b'T' | b' ') {
        return None;
    }
    let time = &t[11..];
    let b = time.as_bytes();
    let ok = b[2] == b':'
        && b[5] == b':'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 2 || i == 5 || c.is_ascii_digit())
        && time[..2].parse::<u32>().ok()? < 24
        && time[3..5].parse::<u32>().ok()? < 60
        && time[6..8].parse::<u32>().ok()? < 60;
    ok.then(|| format!("{}T{time}Z", &t[..10]))
}

/// Infers one column's type from its values (empty cells make it nullable).
pub fn infer(header: &str, values: &[&str]) -> (ColumnType, bool) {
    let present: Vec<&str> = values
        .iter()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .collect();
    let nullable = present.len() != values.len();
    if present.is_empty() {
        return (ColumnType::Text, true);
    }
    if present
        .iter()
        .all(|v| v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("false"))
    {
        return (ColumnType::Boolean, nullable);
    }
    let numbers: Option<Vec<(bool, String, String)>> = present.iter().map(|v| number(v)).collect();
    if let Some(numbers) = numbers {
        let places = numbers.iter().map(|(_, _, f)| f.len()).max().unwrap_or(0);
        let dollars = present.iter().any(|v| v.contains('$'));
        if places <= 2 && (dollars || money_hint(header)) {
            return (ColumnType::Money, nullable);
        }
        if places == 0 {
            return (ColumnType::Integer, nullable);
        }
        if places <= 6 {
            return (ColumnType::Decimal(places as u32), nullable);
        }
    }
    if present.iter().all(|v| is_date(v)) {
        return (ColumnType::Date, nullable);
    }
    if present.iter().all(|v| timestamp(v).is_some()) {
        return (ColumnType::Timestamp, nullable);
    }
    (ColumnType::Text, nullable)
}

/// The inferred columns of a table.
pub fn columns(table: &Table) -> Vec<Column> {
    let ids = unique_ids(&table.header);
    table
        .header
        .iter()
        .enumerate()
        .map(|(index, header)| {
            let values: Vec<&str> = table
                .rows
                .iter()
                .map(|row| row.get(index).map(String::as_str).unwrap_or(""))
                .collect();
            let (kind, nullable) = infer(header, &values);
            Column {
                id: ids[index].clone(),
                header: header.clone(),
                kind,
                nullable,
            }
        })
        .collect()
}

/// The preview the canvas shows (inferred defaults, at most 20 sample rows).
pub fn preview(table: &Table, columns: &[Column]) -> Value {
    json!({
        "format": table.format,
        "sheet": table.sheet,
        "rows": table.rows.len(),
        "columns": columns.iter().map(Column::to_json).collect::<Vec<_>>(),
        "sample": table.rows.iter().take(super::MAX_SAMPLE_ROWS).collect::<Vec<_>>(),
    })
}

/// One cell in its canonical text, or `None` when it does not fit.
pub fn canonical(kind: ColumnType, nullable: bool, raw: &str) -> Option<String> {
    let text = raw.trim();
    if text.is_empty() {
        return nullable.then(String::new);
    }
    Some(match kind {
        ColumnType::Text => raw.to_owned(),
        ColumnType::Boolean => match text.to_ascii_lowercase().as_str() {
            "true" => "true".into(),
            "false" => "false".into(),
            _ => return None,
        },
        ColumnType::Integer => {
            let (negative, whole, fraction) = number(text)?;
            if !fraction.is_empty() {
                return None;
            }
            format!("{}{whole}", if negative && whole != "0" { "-" } else { "" })
        }
        ColumnType::Decimal(scale) => {
            let (negative, whole, fraction) = number(text)?;
            if fraction.len() > scale as usize {
                return None;
            }
            let sign = if negative { "-" } else { "" };
            if fraction.is_empty() {
                format!("{sign}{whole}")
            } else {
                format!("{sign}{whole}.{fraction}")
            }
        }
        ColumnType::Money => {
            let (negative, whole, fraction) = number(text)?;
            if fraction.len() > 2 {
                return None;
            }
            format!(
                "{}{whole}.{:0<2}",
                if negative { "-" } else { "" },
                fraction
            )
        }
        ColumnType::Date => is_date(text).then(|| text.to_owned())?,
        ColumnType::Timestamp => timestamp(text)?,
    })
}

fn csv_field(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

/// The canonical CSV of a table under confirmed columns; `Err(column id)`
/// names the first column with a cell that does not fit.
pub fn canonical_csv(table: &Table, columns: &[Column]) -> Result<Vec<u8>, String> {
    if columns.len() != table.header.len() || table.rows.is_empty() {
        return Err(columns.first().map(|c| c.id.clone()).unwrap_or_default());
    }
    let mut out = String::new();
    out.push_str(
        &columns
            .iter()
            .map(|c| c.id.as_str())
            .collect::<Vec<_>>()
            .join(","),
    );
    out.push('\n');
    for row in &table.rows {
        let mut cells = Vec::with_capacity(columns.len());
        for (index, column) in columns.iter().enumerate() {
            let raw = row.get(index).map(String::as_str).unwrap_or("");
            let cell =
                canonical(column.kind, column.nullable, raw).ok_or_else(|| column.id.clone())?;
            cells.push(csv_field(&cell));
        }
        out.push_str(&cells.join(","));
        out.push('\n');
    }
    Ok(out.into_bytes())
}

/// The member's confirmation (`[{"id","type","nullable","scale"?}]`) over the
/// inferred columns: the same ids in the same order; types and nullability
/// as confirmed.
pub fn confirmed(inferred: &[Column], body: &Value) -> Option<Vec<Column>> {
    let list = body.as_array()?;
    if list.len() != inferred.len() {
        return None;
    }
    list.iter()
        .zip(inferred)
        .map(|(item, column)| {
            let object: &Map<String, Value> = item.as_object()?;
            if !object
                .keys()
                .all(|k| matches!(k.as_str(), "id" | "type" | "nullable" | "scale"))
                || object.get("id")?.as_str()? != column.id
            {
                return None;
            }
            let scale = match object.get("scale") {
                None | Some(Value::Null) => None,
                Some(v) => Some(v.as_u64()?),
            };
            Some(Column {
                id: column.id.clone(),
                header: column.header.clone(),
                kind: ColumnType::parse(object.get("type")?.as_str()?, scale)?,
                nullable: object.get("nullable")?.as_bool()?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(header: &[&str], rows: &[&[&str]]) -> Table {
        Table {
            format: "csv",
            sheet: None,
            header: header.iter().map(|s| s.to_string()).collect(),
            rows: rows
                .iter()
                .map(|r| r.iter().map(|s| s.to_string()).collect())
                .collect(),
        }
    }

    fn view_document(
        kind: &str,
        rows: usize,
        columns: usize,
        offset: usize,
        column_offset: usize,
        cells: Value,
    ) -> Value {
        json!({"status":"parsed","profile":"view","result":{
            "format":kind,"selected_sheet":"Other","row_count":rows,"column_count":columns,
            "row_offset":offset,"column_offset":column_offset,"page_rows":50,"page_columns":32,
            "formula_count":0,"missing_formula_results":0,"formula_cells":[],"cells":cells}})
    }

    #[test]
    fn original_pages_preserve_coordinates_lexical_values_and_whole_cells() {
        let doc = view_document(
            "csv",
            104,
            153,
            50,
            128,
            json!([
                {"coordinate":{"type":"csv","logical_record":52},"column":130,
                 "original":"<script>inert</script>\n001.2300","value":{"type":"integer","value":"999"}}
            ]),
        );
        let page = original_page_of(&doc, "csv", vec![], 0, 50, 128).unwrap();
        assert_eq!(page["row_count"], 104);
        assert_eq!(page["column_count"], 153);
        assert_eq!(page["column_offset"], 128);
        assert_eq!(page["column_page_size"], 32);
        assert_eq!(page["rows"].as_array().unwrap().len(), 50);
        assert_eq!(page["rows"][1].as_array().unwrap().len(), 25);
        assert_eq!(page["rows"][1][1], "<script>inert</script>\n001.2300");
        assert_eq!(page["rows"][0][0], "");
        let doc = view_document("csv", 104, 153, 100, 128, json!([]));
        assert_eq!(
            original_page_of(&doc, "csv", vec![], 0, 100, 128).unwrap()["rows"]
                .as_array()
                .unwrap()
                .len(),
            4
        );
        let doc = view_document(
            "csv",
            1,
            1,
            0,
            0,
            json!([{"coordinate":{"type":"csv","logical_record":1},"column":1,"original":"  Header  "}]),
        );
        assert_eq!(
            original_page_of(&doc, "csv", vec![], 0, 0, 0).unwrap()["rows"][0][0],
            "  Header  "
        );
        let doc = view_document(
            "xlsx",
            3,
            2,
            0,
            0,
            json!([{"coordinate":{"type":"xlsx","sheet":"Other","row":3,"column":2},"original":"-1.25e2"}]),
        );
        let page =
            original_page_of(&doc, "xlsx", vec!["First".into(), "Other".into()], 1, 0, 0).unwrap();
        assert_eq!(page["rows"], json!([["", ""], ["", ""], ["", "-1.25e2"]]));
    }

    #[test]
    fn original_pages_reject_wrong_windows_coordinates_and_unbounded_metadata() {
        let base = view_document(
            "csv",
            100,
            153,
            50,
            128,
            json!([{"coordinate":{"type":"csv","logical_record":51},"column":129,"original":"ok"}]),
        );
        assert!(original_page_of(&base, "csv", vec![], 0, 50, 128).is_some());
        assert!(original_page_of(&base, "csv", vec![], 0, 0, 128).is_none());
        assert!(original_page_of(&base, "csv", vec![], 0, 50, 0).is_none());
        for (field, value) in [
            ("row_count", json!(1_048_577)),
            ("column_count", json!(16_385)),
            ("row_count", json!(99.5)),
            ("page_rows", json!(51)),
            ("page_columns", json!(33)),
            ("row_offset", json!(0)),
            ("column_offset", json!(0)),
            ("formula_count", json!(1)),
        ] {
            let mut doc = base.clone();
            doc["result"][field] = value;
            assert!(
                original_page_of(&doc, "csv", vec![], 0, 50, 128).is_none(),
                "{field}"
            );
        }
        for (row, column) in [(50, 129), (101, 129), (51, 128), (51, 154), (0, 129)] {
            let mut doc = base.clone();
            doc["result"]["cells"][0]["coordinate"]["logical_record"] = json!(row);
            doc["result"]["cells"][0]["column"] = json!(column);
            assert!(original_page_of(&doc, "csv", vec![], 0, 50, 128).is_none());
        }
        let mut doc = base.clone();
        let duplicate = doc["result"]["cells"][0].clone();
        doc["result"]["cells"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
        assert!(original_page_of(&doc, "csv", vec![], 0, 50, 128).is_none());
        let mut doc = base.clone();
        doc["profile"] = json!("demo");
        assert!(original_page_of(&doc, "csv", vec![], 0, 50, 128).is_none());
        let mut doc = base.clone();
        doc["result"]["cells"][0]["original"] = json!("x".repeat(ORIGINAL_MAX_CELL_BYTES + 1));
        assert!(original_page_of(&doc, "csv", vec![], 0, 50, 128).is_none());
        let mut doc = base.clone();
        doc["result"]["cells"][0]["original"] = json!("x".repeat(ORIGINAL_MAX_CELL_CHARS + 1));
        assert!(original_page_of(&doc, "csv", vec![], 0, 50, 128).is_none());
        doc["result"]["cells"][0]["original"] = json!("🗂".repeat(ORIGINAL_MAX_CELL_CHARS));
        assert!(original_page_of(&doc, "csv", vec![], 0, 50, 128).is_some());
        let empty = view_document("csv", 0, 0, 0, 0, json!([]));
        assert_eq!(
            original_page_of(&empty, "csv", vec![], 0, 0, 0).unwrap()["rows"],
            json!([])
        );
    }

    #[test]
    fn original_pages_identify_missing_formula_results_without_inventing_values() {
        let mut doc = view_document(
            "xlsx",
            3,
            2,
            0,
            0,
            json!([
                {"coordinate":{"type":"xlsx","sheet":"Other","row":2,"column":1},"original":"10.00"},
                {"coordinate":{"type":"xlsx","sheet":"Other","row":3,"column":2},"original":null}
            ]),
        );
        doc["result"]["formula_count"] = json!(2);
        doc["result"]["missing_formula_results"] = json!(1);
        doc["result"]["formula_cells"] =
            json!([{"row":2,"column":1,"cached":true},{"row":3,"column":2,"cached":false}]);
        let render = |d: &Value| original_page_of(d, "xlsx", vec!["Other".into()], 0, 0, 0);
        let page = render(&doc).unwrap();
        assert_eq!(page["rows"][1][0], "10.00");
        assert_eq!(page["rows"][2][1], "");
        assert_eq!(page["formula_count"], 2);
        assert_eq!(page["missing_formula_results"], 1);
        for value in [json!(-1), json!(3), json!(1.5)] {
            let mut bad = doc.clone();
            bad["result"]["missing_formula_results"] = value;
            assert!(render(&bad).is_none());
        }
        let mut bad = doc.clone();
        bad["result"]["cells"][1]["original"] = json!("invented");
        assert!(render(&bad).is_none());
        let mut bad = doc.clone();
        bad["result"]["formula_cells"][1] = bad["result"]["formula_cells"][0].clone();
        assert!(render(&bad).is_none());
        let mut bad = doc.clone();
        bad["result"]["formula_cells"][1]["row"] = json!(4);
        assert!(render(&bad).is_none());
        let mut bad = doc.clone();
        bad["result"]["cells"][0]["coordinate"]["sheet"] = json!("wrong");
        assert!(render(&bad).is_none());
    }

    #[test]
    fn infers_the_closed_types() {
        assert_eq!(
            infer("Revenue", &["1250.00", "980.5"]),
            (ColumnType::Money, false)
        );
        assert_eq!(infer("x", &["$1,250", "$99"]), (ColumnType::Money, false));
        assert_eq!(
            infer("Signups", &["12", "1,204", ""]),
            (ColumnType::Integer, true)
        );
        assert_eq!(
            infer("Churn rate", &["0.031", "0.0275"]),
            (ColumnType::Decimal(4), false)
        );
        assert_eq!(
            infer("Month", &["2026-07-01", "2026-08-01"]),
            (ColumnType::Date, false)
        );
        assert_eq!(
            infer("At", &["2026-07-01 09:00:00", "2026-07-02T10:00:00Z"]),
            (ColumnType::Timestamp, false)
        );
        assert_eq!(
            infer("Active", &["true", "FALSE"]),
            (ColumnType::Boolean, false)
        );
        assert_eq!(infer("Region", &["North", "1"]), (ColumnType::Text, false));
        assert_eq!(infer("Empty", &["", ""]), (ColumnType::Text, true));
        assert_eq!(infer("Code", &["1,23"]), (ColumnType::Text, false));
    }

    #[test]
    fn ids_are_local_and_unique() {
        let t = table(
            &["Revenue ($)", "revenue", "1st", "", "Région"],
            &[&["1", "2", "3", "4", "5"]],
        );
        let ids: Vec<String> = columns(&t).into_iter().map(|c| c.id).collect();
        assert_eq!(ids, ["revenue", "revenue_2", "c_1st", "column_4", "r_gion"]);
        assert!(ids.iter().all(|id| crate::storage::valid_local_id(id)));
    }

    #[test]
    fn canonical_csv_follows_the_normalization_table() {
        let t = table(
            &["Month", "Revenue", "Note"],
            &[
                &["2026-07-01", "$1,250", "a, \"b\""],
                &["2026-08-01", "980.5", ""],
            ],
        );
        let mut cols = columns(&t);
        assert_eq!(cols[1].kind, ColumnType::Money);
        assert!(cols[2].nullable);
        let csv = String::from_utf8(canonical_csv(&t, &cols).unwrap()).unwrap();
        assert_eq!(
            csv,
            "month,revenue,note\n2026-07-01,1250.00,\"a, \"\"b\"\"\"\n2026-08-01,980.50,\n"
        );
        // A confirmed type the values do not fit names the column.
        cols[0].kind = ColumnType::Integer;
        assert_eq!(canonical_csv(&t, &cols), Err("month".into()));
        cols[0].kind = ColumnType::Date;
        cols[2].nullable = false;
        assert_eq!(canonical_csv(&t, &cols), Err("note".into()));
    }

    #[test]
    fn confirmation_keeps_ids_and_order() {
        let t = table(&["A", "B"], &[&["1", "x"]]);
        let inferred = columns(&t);
        let ok = json!([{"id": "a", "type": "decimal", "nullable": true, "scale": 2}, {"id": "b", "type": "text", "nullable": false}]);
        let got = confirmed(&inferred, &ok).unwrap();
        assert_eq!(got[0].kind, ColumnType::Decimal(2));
        assert!(confirmed(&inferred, &json!([{"id": "b", "type": "text", "nullable": false}, {"id": "a", "type": "text", "nullable": false}])).is_none());
        assert!(confirmed(&inferred, &json!([{"id": "a", "type": "sql", "nullable": false}, {"id": "b", "type": "text", "nullable": false}])).is_none());
        assert!(confirmed(&inferred, &json!([{"id": "a", "type": "text", "nullable": false, "x": 1}, {"id": "b", "type": "text", "nullable": false}])).is_none());
    }

    #[test]
    fn parser_documents_become_tables() {
        let doc = json!({"status": "parsed", "result": {"cells": [
            {"coordinate": {"type": "csv", "logical_record": 1}, "column": 1, "value": {"type": "text", "value": "a"}},
            {"coordinate": {"type": "csv", "logical_record": 1}, "column": 2, "value": {"type": "text", "value": "b"}},
            {"coordinate": {"type": "csv", "logical_record": 2}, "column": 1, "value": {"type": "text", "value": "1"}},
            {"coordinate": {"type": "xlsx", "row": 3, "column": 2}, "value": {"type": "decimal", "coefficient": "-5", "scale": 2}},
        ]}});
        let t = table_of(&doc, "csv", None).unwrap();
        assert_eq!(t.header, ["a", "b"]);
        assert_eq!(
            t.rows,
            vec![
                vec!["1".to_string(), String::new()],
                vec![String::new(), "-0.05".to_string()]
            ]
        );
    }
}
