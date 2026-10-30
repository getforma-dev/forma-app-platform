//! Server-side resolution of a [`Parsed`] action into a storage-validated
//! command draft (module notes in `actions`). Every identity here comes from
//! storage under the member's own authority: the app from the member's
//! library, the saved work from the member's listing (or a new one the member
//! creates), the approver from the members list, the target record from the
//! records the member may read. The model's words are hints and values only.
use super::{prompt::CatalogApp, Kind, Parsed};
use crate::{
    auth::capability,
    commands::routes::typed_value,
    directory, events,
    storage::{
        composer_bindings, CommandRequest, DryRun, Error, LibraryApp, Member, OpenableWork,
        RecordRow, RecordsListing, Stores, TrustedRequest,
    },
};
use serde_json::{json, Value};

/// A saved work's context text bound (`storage::definitions`).
const MAX_CONTEXT_CHARACTERS: usize = 500;

/// One value of the draft, typed and displayed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DraftValue {
    pub field: String,
    pub label: String,
    pub display: String,
    pub value: Value,
}
/// The approver the server resolved (a same-organization member).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Approver {
    pub principal: String,
    pub label: String,
}
/// The existing record a decision targets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Target {
    pub record_id: String,
    pub version: i64,
    pub summary: String,
}
/// A confirmation card: the typed command storage would accept, and what the
/// member sees before confirming.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Draft {
    pub kind: Kind,
    pub app: String,
    pub work: String,
    pub title: String,
    /// The app's saved work was created for this member just now ("the first
    /// time").
    pub placed: bool,
    pub collection: String,
    pub request: CommandRequest,
    /// The whole record after the command, as storage composed it.
    pub values: Vec<DraftValue>,
    pub approver: Option<Approver>,
    pub target: Option<Target>,
    /// A random correlation id the canvas echoes in its log line; it grants
    /// nothing (the command route re-judges the request on its own).
    pub confirm_token: String,
}

/// What an action resolved to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Resolution {
    Draft(Box<Draft>),
    /// Show this saved work (placed now when `placed`).
    Open {
        app: String,
        work: String,
        title: String,
        placed: bool,
    },
    /// Not a record action: the canvas composes instead.
    Place,
}

/// Why no draft was produced; the route maps each to a design §9 answer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Refusal {
    /// A denial anywhere: the uniform 404.
    NotFound,
    StoreUnavailable,
    /// Neutral 422 `could_not_draft`; the code is for the log only.
    CouldNotDraft(&'static str),
}
impl From<Error> for Refusal {
    fn from(error: Error) -> Self {
        match error {
            Error::Denied => Self::NotFound,
            _ => Self::StoreUnavailable,
        }
    }
}

fn fresh_key() -> Option<String> {
    let bytes = capability::random_bytes::<16>()?;
    Some(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// A typed value as people read it: money as dollars, null as a dash,
/// everything else as its text.
pub fn display(value: &Value) -> String {
    let kind = value.get("type").and_then(Value::as_str).unwrap_or("");
    let text = |key: &str| {
        value
            .get(key)
            .map(|v| match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .unwrap_or_default()
    };
    match kind {
        "null" => "—".into(),
        "money" => {
            let minor = text("minor_units");
            let (negative, digits) = match minor.strip_prefix('-') {
                Some(rest) => (true, rest.to_owned()),
                None => (false, minor),
            };
            let padded = format!("{digits:0>3}");
            let (whole, cents) = padded.split_at(padded.len() - 2);
            format!(
                "{}${}.{cents}",
                if negative { "-" } else { "" },
                crate::query::page::display(whole)
            )
        }
        "decimal" => text("coefficient"),
        "reference" => text("record_id"),
        _ => text("value"),
    }
}

/// A dollar amount ("86", "86.5", "1,234.00") as minor units ("8600").
pub fn cents(raw: &str) -> Option<String> {
    let cleaned: String = raw
        .trim()
        .trim_start_matches('$')
        .chars()
        .filter(|c| *c != ',' && *c != ' ')
        .collect();
    let cleaned = cleaned
        .trim_end_matches("USD")
        .trim_end_matches("usd")
        .trim_end_matches("dollars")
        .trim();
    let (negative, cleaned) = match cleaned.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, cleaned),
    };
    let (whole, fraction) = cleaned.split_once('.').unwrap_or((cleaned, ""));
    if whole.is_empty() && fraction.is_empty() {
        return None;
    }
    if !(whole.bytes().all(|b| b.is_ascii_digit()) && fraction.bytes().all(|b| b.is_ascii_digit()))
        || whole.len() > 12
        || fraction.len() > 2
    {
        return None;
    }
    let whole: u64 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    let fraction: u64 = format!("{fraction:0<2}").parse().ok()?;
    let minor = whole * 100 + fraction;
    if minor == 0 {
        return Some("0".into());
    }
    Some(format!("{}{minor}", if negative { "-" } else { "" }))
}

/// A `YYYY-MM-DD` text with plausible parts.
fn date_shaped(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(i, b)| matches!(i, 4 | 7) || b.is_ascii_digit())
        && (1..=12).contains(&value[5..7].parse::<u8>().unwrap_or(0))
        && (1..=31).contains(&value[8..10].parse::<u8>().unwrap_or(0))
}

fn label(field: &str) -> String {
    let mut out = field.replace('_', " ");
    if let Some(first) = out.get(0..1) {
        let upper = first.to_uppercase();
        out.replace_range(0..1, &upper);
    }
    out
}

/// The library app for a parse: the hinted id, else the first app that files
/// records of the kind asked for.
fn choose_app<'a>(catalog: &'a [CatalogApp], parsed: &Parsed) -> Option<&'a CatalogApp> {
    if !parsed.app_hint.is_empty() {
        if let Some(app) = catalog.iter().find(|app| app.app == parsed.app_hint) {
            return Some(app);
        }
    }
    match parsed.kind {
        Kind::Decide => catalog.iter().find(|app| app.decide_command().is_some()),
        _ => catalog
            .iter()
            .find(|app| {
                app.create_command().is_some()
                    && parsed.values.keys().all(|field| app.field(field).is_some())
            })
            .or_else(|| catalog.iter().find(|app| app.create_command().is_some())),
    }
}

/// The member's saved work of `app`: a writable one, else any readable one,
/// else a new one bound under the member's own authority (the composer's
/// `retrieved` path).
async fn find_or_place_work(
    stores: &Stores,
    member: &TrustedRequest,
    library: &LibraryApp,
    utterance: &str,
) -> Result<(OpenableWork, bool), Refusal> {
    let listed = stores.data.openable_works(member).await?;
    let mut works: Vec<OpenableWork> = listed
        .into_iter()
        .filter(|work| work.app == library.app)
        .collect();
    works.sort_by_key(|work| !work.can_write);
    if let Some(work) = works.into_iter().next() {
        return Ok((work, false));
    }
    let context = stores.workspace.composer_context(member).await?;
    let bindings = composer_bindings(&library.inputs, &library.record_schemas, &context.sources)
        .ok_or(Refusal::CouldNotDraft("unbindable"))?;
    let context_text: String = utterance.chars().take(MAX_CONTEXT_CHARACTERS).collect();
    let saved = stores
        .workspace
        .create_work(
            member,
            &library.app,
            &library.publication,
            &context_text,
            &bindings,
        )
        .await?;
    events::hub().app_placed(member.organization(), &library.app, &saved.work);
    tracing::info!(
        event = "action.placed",
        organization_id = member.organization(),
        principal_id = member.principal(),
        app_id = library.app,
        work_id = saved.work,
        origin = "retrieved",
        "library app instantiated for the action"
    );
    let placed = stores
        .data
        .openable_works(member)
        .await?
        .into_iter()
        .find(|work| work.work == saved.work)
        .ok_or(Refusal::StoreUnavailable)?;
    Ok((placed, true))
}

fn tokens(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|w| w.len() >= 2)
        .map(|w| w.to_lowercase().trim_end_matches('s').to_owned())
        .collect()
}

/// The approver: the other member whose display name best matches `name`
/// (whole name, then first name, then a token prefix); none when nothing
/// matches. Labels without a display name never match a name.
pub fn match_approver(name: &str, members: &[Member]) -> Option<Approver> {
    let wanted = tokens(name);
    if wanted.is_empty() {
        return None;
    }
    let mut best: Option<(u32, &Member, String)> = None;
    for member in members {
        let Some(display) = directory::display_name(&member.principal) else {
            continue;
        };
        let have = tokens(&display);
        let score = if have == wanted {
            4
        } else if wanted.iter().all(|w| have.contains(w)) {
            3
        } else if have
            .first()
            .is_some_and(|first| wanted.first() == Some(first))
        {
            2
        } else if wanted.iter().any(|w| {
            have.iter()
                .any(|h| h.starts_with(w.as_str()) && w.len() >= 3)
        }) {
            1
        } else {
            0
        };
        if score > 0 && best.as_ref().is_none_or(|(s, _, _)| score > *s) {
            best = Some((score, member, display));
        }
    }
    best.map(|(_, member, display)| Approver {
        principal: member.principal.clone(),
        label: display,
    })
}

fn field_value<'a>(record: &'a RecordRow, field: &str) -> Option<&'a Value> {
    record
        .values
        .as_array()?
        .iter()
        .find(|item| item.get("field_id").and_then(Value::as_str) == Some(field))
        .and_then(|item| item.get("value"))
}

/// A record as one line: its text and money values.
pub fn summarize(record: &RecordRow) -> String {
    let parts: Vec<String> = record
        .values
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let value = item.get("value")?;
            let kind = value.get("type").and_then(Value::as_str)?;
            (kind != "null" && kind != "enum").then(|| display(value))
        })
        .collect();
    parts.join(" · ")
}

/// The pending record a decision names: among records whose transition field
/// is at a declared `from`, the one whose values share the most words with
/// the hint; the newest when there is no hint or no overlap.
pub fn match_target<'a>(
    hint: Option<&str>,
    records: &'a [RecordRow],
    field: &str,
    froms: &[&str],
) -> Option<&'a RecordRow> {
    let candidates: Vec<&RecordRow> = records
        .iter()
        .filter(|record| {
            field_value(record, field)
                .and_then(|v| v.get("value"))
                .and_then(Value::as_str)
                .is_some_and(|current| froms.contains(&current))
        })
        .collect();
    let wanted = hint.map(tokens).unwrap_or_default();
    if wanted.is_empty() {
        return candidates.first().copied();
    }
    let mut best: Option<(usize, &RecordRow)> = None;
    for record in &candidates {
        let mut haystack = tokens(&summarize(record));
        haystack.extend(tokens(&directory::label(&record.creator)));
        // The amount as dollars and as cents both count.
        if let Some(amount) = field_value(record, "amount")
            .and_then(|v| v.get("minor_units"))
            .and_then(Value::as_str)
        {
            haystack.push(amount.to_owned());
        }
        let score = wanted.iter().filter(|w| haystack.contains(w)).count();
        if score > 0 && best.is_none_or(|(s, _)| score > s) {
            best = Some((score, record));
        }
    }
    best.map(|(_, record)| record)
        .or_else(|| candidates.first().copied())
}

/// Builds the typed values of a `create`: every writable field from the
/// parse under the published spec (money in cents, dates checked, enums
/// normalized, the status forced to the create's starting value), nullable
/// fields omitted when absent.
fn create_values(
    app: &CatalogApp,
    listing: &RecordsListing,
    collection_schema: &str,
    parsed: &Parsed,
    today: &str,
) -> Result<Value, Refusal> {
    let summary = listing
        .collections
        .iter()
        .find(|c| c.schema == collection_schema)
        .ok_or(Refusal::CouldNotDraft("no_collection"))?;
    let command = app
        .create_command()
        .ok_or(Refusal::CouldNotDraft("no_create_command"))?;
    let mut values = Vec::new();
    for field in &summary.fields {
        if !command.writable.contains(&field.id) {
            continue;
        }
        let spec = &field.value_type;
        let kind = spec.get("type").and_then(Value::as_str).unwrap_or("");
        let raw = parsed
            .values
            .get(&field.id)
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        let text: String = match kind {
            "enum" => {
                let options: Vec<&str> = spec
                    .get("values")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect();
                // The status of a new record is the declared starting state
                // (the `from` of the decide transitions), never the model's.
                let starting = app
                    .decide_command()
                    .and_then(|decide| decide.transitions.iter().find(|(f, _, _)| f == &field.id))
                    .map(|(_, from, _)| from.as_str());
                if let Some(start) = starting {
                    start.to_owned()
                } else {
                    let lower = raw.to_lowercase();
                    options
                        .iter()
                        .find(|o| **o == lower)
                        .or_else(|| options.iter().find(|o| **o == "other"))
                        .or_else(|| options.first())
                        .map(|o| (*o).to_owned())
                        .ok_or(Refusal::CouldNotDraft("no_enum_value"))?
                }
            }
            "money" => match cents(raw) {
                Some(minor) => minor,
                None if field.nullable => String::new(),
                None => return Err(Refusal::CouldNotDraft("missing_amount")),
            },
            "date" => {
                if date_shaped(raw) {
                    raw.to_owned()
                } else {
                    today.to_owned()
                }
            }
            "text" => {
                if raw.is_empty() && !field.nullable {
                    return Err(Refusal::CouldNotDraft("missing_text"));
                }
                raw.chars().take(200).collect()
            }
            "integer" | "decimal" | "boolean" | "timestamp" => raw.to_owned(),
            _ => return Err(Refusal::CouldNotDraft("unsupported_field")),
        };
        if text.is_empty() && field.nullable {
            continue;
        }
        let typed = typed_value(spec, &text).ok_or(Refusal::CouldNotDraft("untypable_value"))?;
        values.push(json!({"field_id": field.id, "value": typed}));
    }
    Ok(Value::Array(values))
}

/// Resolves `parsed` for `member` (module notes). `today` is the member's
/// date for defaults.
pub async fn resolve(
    stores: &Stores,
    member: &TrustedRequest,
    library: &[LibraryApp],
    catalog: &[CatalogApp],
    parsed: &Parsed,
    utterance: &str,
    today: &str,
) -> Result<Resolution, Refusal> {
    if parsed.kind == Kind::Place {
        return Ok(Resolution::Place);
    }
    let app = choose_app(catalog, parsed).ok_or(Refusal::CouldNotDraft("no_record_app"))?;
    let library_app = library
        .iter()
        .find(|l| l.app == app.app)
        .ok_or(Refusal::CouldNotDraft("no_record_app"))?;
    let (work, placed) = find_or_place_work(stores, member, library_app, utterance).await?;
    if parsed.kind == Kind::Open {
        return Ok(Resolution::Open {
            app: work.app,
            work: work.work,
            title: work.title,
            placed,
        });
    }
    if !work.can_write {
        return Err(Refusal::CouldNotDraft("no_record_write"));
    }
    let listing = stores
        .data
        .list_records(member, &work.app, &work.work)
        .await?;
    let summary = listing
        .collections
        .iter()
        .find(|c| c.schema == app.schema)
        .ok_or(Refusal::CouldNotDraft("no_collection"))?;
    let key = fresh_key().ok_or(Refusal::StoreUnavailable)?;
    let confirm_token = fresh_key().ok_or(Refusal::StoreUnavailable)?;
    let (command, record, expected_version, values, target) = match parsed.kind {
        Kind::Create => {
            let command = app
                .create_command()
                .ok_or(Refusal::CouldNotDraft("no_create_command"))?;
            let record = capability::random_uuid().ok_or(Refusal::StoreUnavailable)?;
            let values = create_values(app, &listing, &app.schema, parsed, today)?;
            (command.id.clone(), record, 0, values, None)
        }
        Kind::Decide => {
            let command = app
                .decide_command()
                .ok_or(Refusal::CouldNotDraft("no_decide_command"))?;
            let field = command
                .transitions
                .first()
                .map(|(f, _, _)| f.clone())
                .ok_or(Refusal::CouldNotDraft("no_transition"))?;
            let froms: Vec<&str> = command
                .transitions
                .iter()
                .filter(|(f, _, _)| *f == field)
                .map(|(_, from, _)| from.as_str())
                .collect();
            let candidates: Vec<RecordRow> = listing
                .records
                .iter()
                .filter(|r| r.schema == app.schema)
                .cloned()
                .collect();
            let record = match_target(
                parsed.target_record_hint.as_deref(),
                &candidates,
                &field,
                &froms,
            )
            .ok_or(Refusal::CouldNotDraft("no_pending_record"))?;
            let current = field_value(record, &field)
                .and_then(|v| v.get("value"))
                .and_then(Value::as_str)
                .unwrap_or("");
            let wanted = parsed
                .values
                .get(&field)
                .and_then(Value::as_str)
                .map(|s| s.trim().to_lowercase());
            let tos: Vec<&str> = command
                .transitions
                .iter()
                .filter(|(f, from, _)| *f == field && from == current)
                .map(|(_, _, to)| to.as_str())
                .collect();
            let to = match wanted.as_deref() {
                Some(w) if tos.contains(&w) => w.to_owned(),
                Some(w) if w.starts_with("approv") || w.starts_with("accept") => tos
                    .iter()
                    .find(|t| t.starts_with("approv") || t.starts_with("accept"))
                    .map(|t| (*t).to_owned())
                    .ok_or(Refusal::CouldNotDraft("no_transition"))?,
                Some(w)
                    if w.starts_with("reject")
                        || w.starts_with("declin")
                        || w.starts_with("den") =>
                {
                    tos.iter()
                        .find(|t| {
                            t.starts_with("reject")
                                || t.starts_with("declin")
                                || t.starts_with("den")
                        })
                        .map(|t| (*t).to_owned())
                        .ok_or(Refusal::CouldNotDraft("no_transition"))?
                }
                _ => tos
                    .first()
                    .map(|t| (*t).to_owned())
                    .ok_or(Refusal::CouldNotDraft("no_transition"))?,
            };
            let spec = summary
                .fields
                .iter()
                .find(|f| f.id == field)
                .map(|f| f.value_type.clone())
                .ok_or(Refusal::CouldNotDraft("no_transition"))?;
            let mut values = vec![
                json!({"field_id": field, "value": typed_value(&spec, &to).ok_or(Refusal::CouldNotDraft("untypable_value"))?}),
            ];
            if let Some(note) = parsed.values.get("note").and_then(Value::as_str) {
                if command.writable.iter().any(|w| w == "note") {
                    if let Some(note_spec) = summary.fields.iter().find(|f| f.id == "note") {
                        let text: String = note.trim().chars().take(200).collect();
                        if !text.is_empty() {
                            values.push(json!({"field_id": "note", "value": typed_value(&note_spec.value_type, &text).ok_or(Refusal::CouldNotDraft("untypable_value"))?}));
                        }
                    }
                }
            }
            let target = Target {
                record_id: record.record_id.clone(),
                version: record.version,
                summary: summarize(record),
            };
            (
                command.id.clone(),
                record.record_id.clone(),
                record.version,
                Value::Array(values),
                Some(target),
            )
        }
        Kind::Open | Kind::Place => unreachable!("handled above"),
    };
    let request = CommandRequest {
        app: work.app.clone(),
        work: work.work.clone(),
        publication: listing.publication.clone(),
        command,
        schema: summary.schema.clone(),
        schema_version_expected: i64::from(summary.schema_version),
        record,
        expected_record_version: expected_version,
        expected_work_version: listing.work_version,
        values,
        idempotency_key: key,
    };
    let state = match stores.data.dry_run_command(member, &request).await {
        Ok(DryRun::Valid { state, .. }) => state,
        Ok(DryRun::Stale) => return Err(Refusal::CouldNotDraft("stale")),
        Ok(DryRun::Conflict { .. }) => return Err(Refusal::CouldNotDraft("conflict")),
        // A value storage refuses under the published schema is a neutral
        // refusal (the member may write the work: judged above).
        Err(Error::Denied) => return Err(Refusal::CouldNotDraft("refused_by_schema")),
        Err(error) => return Err(error.into()),
    };
    let values: Vec<DraftValue> = state
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let field = item.get("field_id")?.as_str()?.to_owned();
            let value = item.get("value")?.clone();
            Some(DraftValue {
                label: label(&field),
                display: display(&value),
                field,
                value,
            })
        })
        .collect();
    let approver = match (&parsed.kind, parsed.approver_name.as_deref()) {
        (Kind::Create, Some(name)) => {
            let members = stores.workspace.members(member).await?;
            match_approver(name, &members)
        }
        _ => None,
    };
    Ok(Resolution::Draft(Box::new(Draft {
        kind: parsed.kind,
        app: work.app,
        work: work.work,
        title: work.title,
        placed,
        collection: summary.collection.clone(),
        request,
        values,
        approver,
        target,
        confirm_token,
    })))
}

/// APP-100: the decide command of a listing's collection of `record` (the
/// command declaring transitions), its transition field and the record's
/// current value of it.
fn decide_of<'a>(
    listing: &'a RecordsListing,
    record: &RecordRow,
) -> Option<(
    &'a crate::storage::CollectionSummary,
    &'a crate::storage::RecordCommand,
    String,
    String,
)> {
    let summary = listing
        .collections
        .iter()
        .find(|c| c.collection == record.collection && c.schema == record.schema)?;
    let command = summary
        .commands
        .iter()
        .find(|c| !c.transitions.is_empty())?;
    let field = command.transitions.first()?.0.clone();
    let current = field_value(record, &field)
        .and_then(|v| v.get("value"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    Some((summary, command, field, current))
}

/// APP-100: the records of `listing` waiting on a decision by `me`: the
/// reader holds `record_write`, the record's transition field is at a
/// declared `from`, and `me` did not file it (an approver decides other
/// people's claims). Newest first, as listed.
pub fn pending<'a>(listing: &'a RecordsListing, me: &str) -> Vec<&'a RecordRow> {
    if !listing.can_write {
        return Vec::new();
    }
    listing
        .records
        .iter()
        .filter(|record| record.creator != me)
        .filter(|record| {
            decide_of(listing, record).is_some_and(|(_, command, field, current)| {
                command
                    .transitions
                    .iter()
                    .any(|(f, from, _)| *f == field && *from == current)
            })
        })
        .collect()
}

/// APP-100: the typed `decide` command moving exactly `record_id` (no fuzzy
/// matching: a button carries the record id) to the declared target that
/// `wanted` names ("approved"/"rejected", or a prefix of it), with a fresh
/// server-minted key. `NotFound` when the record is not in this listing or
/// the reader cannot write it; `CouldNotDraft("already_decided")` when it is
/// no longer at a declared `from`. Storage judges the command again.
pub fn exact_decision<'a>(
    listing: &'a RecordsListing,
    record_id: &str,
    wanted: &str,
) -> Result<(CommandRequest, &'a RecordRow), Refusal> {
    let record = listing
        .records
        .iter()
        .find(|r| r.record_id == record_id)
        .ok_or(Refusal::NotFound)?;
    if !listing.can_write {
        return Err(Refusal::NotFound);
    }
    let (summary, command, field, current) = decide_of(listing, record).ok_or(Refusal::NotFound)?;
    let tos: Vec<&str> = command
        .transitions
        .iter()
        .filter(|(f, from, _)| *f == field && *from == current)
        .map(|(_, _, to)| to.as_str())
        .collect();
    if tos.is_empty() {
        return Err(Refusal::CouldNotDraft("already_decided"));
    }
    let wanted = wanted.trim().to_lowercase();
    let stem = wanted.get(..wanted.len().min(5)).unwrap_or("");
    let to = tos
        .iter()
        .find(|t| **t == wanted)
        .or_else(|| tos.iter().find(|t| !stem.is_empty() && t.starts_with(stem)))
        .ok_or(Refusal::CouldNotDraft("no_transition"))?;
    let spec = summary
        .fields
        .iter()
        .find(|f| f.id == field)
        .map(|f| f.value_type.clone())
        .ok_or(Refusal::CouldNotDraft("no_transition"))?;
    let typed = typed_value(&spec, to).ok_or(Refusal::CouldNotDraft("untypable_value"))?;
    let key = fresh_key().ok_or(Refusal::StoreUnavailable)?;
    Ok((
        CommandRequest {
            app: listing.app.clone(),
            work: listing.work.clone(),
            publication: listing.publication.clone(),
            command: command.id.clone(),
            schema: summary.schema.clone(),
            schema_version_expected: i64::from(summary.schema_version),
            record: record.record_id.clone(),
            expected_record_version: record.version,
            expected_work_version: listing.work_version,
            values: Value::Array(vec![json!({"field_id": field, "value": typed})]),
            idempotency_key: key,
        },
        record,
    ))
}

/// APP-100: a record's value of `field` as people read it ("—" when absent).
pub fn record_display(record: &RecordRow, field: &str) -> Option<String> {
    field_value(record, field).map(display)
}
