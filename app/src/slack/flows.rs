//! The Slack flows (module notes in `slack`). Every handler here runs after
//! the request's signature was verified; the synchronous parts only shape an
//! acknowledgement, everything that touches storage, the model or Slack's API
//! runs in `tokio::spawn` and answers through `response_url` (or a DM).
use super::{text, Slack};
use crate::{
    actions::{
        prompt::{self, CatalogApp},
        resolve::{self, Draft, Refusal, Resolution},
        routes::ACT_MEMBER,
        Kind, ParseFailure,
    },
    commands::routes::commit,
    demo, directory, events, origins,
    storage::{self, CommandOutcome, CommandResult, RecordRow, TrustedRequest},
};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc, time::Instant};

pub const CONFIRM: &str = "forma_confirm";
pub const CANCEL: &str = "forma_cancel";
pub const APPROVE: &str = "forma_approve";
pub const REJECT: &str = "forma_reject";
/// Works scanned for a button's record or the pending list.
const MAX_WORKS: usize = 32;
/// Pending records shown in one answer.
const MAX_PENDING: usize = 10;
/// The channel label the canvas chip shows (DS-59).
pub const VIA: &str = "Slack";

const HELP: &str = "Tell Forma what you need, for example:\n• `/forma I spent $86 on a client dinner at Nobu last night, Sarah approves my expenses`\n• `/forma what's waiting for my approval`\n• `/forma link` to connect your Slack account, `/forma unlink` to disconnect it";
const LINK_PROMPT: &str = "Connect your Slack account to Forma first: run `/forma link` and open the link while signed in to Forma.";
/// The one answer for anything this member may not see or do (uniform).
const REFUSED: &str = "Forma couldn't do that. It may not exist, or it isn't yours to act on.";
const UNAVAILABLE: &str = "Forma is unavailable right now. Try again in a moment.";

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}
fn canvas_link(state: &Slack) -> String {
    text::link(&format!("{}/canvas", state.origin()), "Open in Forma")
}

/// The sender's link as a `TrustedRequest`, or nothing (link prompt).
async fn member(state: &Slack, team: &str, user: &str) -> Option<TrustedRequest> {
    let stores = state.auth.stores.as_ref()?;
    match stores.workspace.resolve_slack_link(team, user).await {
        Ok(Some(request)) => Some(request),
        Ok(None) => {
            tracing::info!(event = "slack.unlinked", "slack sender has no live link");
            None
        }
        Err(error) => {
            tracing::warn!(event = "slack.link_unavailable", error = %error, "slack link resolution failed");
            None
        }
    }
}

async fn respond(state: &Slack, response_url: &str, message: Value) {
    if let Err(error) = state.api.respond(response_url, &message).await {
        tracing::warn!(
            event = "slack.respond_failed",
            code = error.code(),
            "response_url post failed"
        );
    }
}

fn field<'a>(fields: &'a HashMap<String, String>, name: &str) -> &'a str {
    fields.get(name).map(String::as_str).unwrap_or("")
}

/// `POST /v1/slack/commands` after verification: the acknowledgement body.
pub fn command(state: &Arc<Slack>, fields: &HashMap<String, String>) -> Value {
    let team = field(fields, "team_id").to_owned();
    let user = field(fields, "user_id").to_owned();
    let response_url = field(fields, "response_url").to_owned();
    if !(storage::slack_id(&team) && storage::slack_id(&user)) {
        return text::ephemeral(REFUSED, None, false);
    }
    let said = text::unescape(field(fields, "text").trim());
    let first = said
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match (first.as_str(), said.split_whitespace().count()) {
        ("", _) | ("help", 1) => text::ephemeral(HELP, None, false),
        ("link", 1) => match state.links.mint(&team, &user, super::now()) {
            Some(token) => {
                tracing::info!(event = "slack.link_offered", "link token minted");
                let url = super::link::url(state.origin(), &token);
                text::ephemeral(
                    &format!(
                        "Open this link while signed in to Forma to connect your Slack account (valid 10 minutes, once): {}",
                        if url.starts_with("https://") { format!("<{url}|Connect to Forma>") } else { text::escape(&url) }
                    ),
                    None,
                    false,
                )
            }
            None => text::ephemeral(UNAVAILABLE, None, false),
        },
        ("unlink", 1) => {
            let state = state.clone();
            tokio::spawn(async move { unlink(state, team, user, response_url).await });
            text::ephemeral("Disconnecting…", None, false)
        }
        _ => {
            let Some(utterance) = prompt::utterance(&said).map(str::to_owned) else {
                return text::ephemeral(HELP, None, false);
            };
            if !state.api.allowed_response_url(&response_url) {
                return text::ephemeral(REFUSED, None, false);
            }
            let state = state.clone();
            tokio::spawn(async move { act(state, team, user, utterance, response_url).await });
            text::ephemeral("Working on it…", None, false)
        }
    }
}

async fn unlink(state: Arc<Slack>, team: String, user: String, response_url: String) {
    let message = match member(&state, &team, &user).await {
        None => text::ephemeral("This Slack account isn't connected to Forma.", None, false),
        Some(member) => match state.auth.stores.as_ref() {
            Some(stores) => match stores.workspace.unlink_slack(&member, &team, &user).await {
                Ok(_) => {
                    tracing::info!(
                        event = "slack.unlinked_by_user",
                        organization_id = member.organization(),
                        principal_id = member.principal(),
                        "slack link revoked"
                    );
                    text::ephemeral(
                        "Disconnected. Run `/forma link` to connect again.",
                        None,
                        false,
                    )
                }
                Err(_) => text::ephemeral(UNAVAILABLE, None, false),
            },
            None => text::ephemeral(UNAVAILABLE, None, false),
        },
    };
    respond(&state, &response_url, message).await;
}

/// The draft as a confirmation card.
fn draft_card(state: &Slack, draft: &Draft, id: &str) -> Value {
    let pairs: Vec<(String, String)> = draft
        .values
        .iter()
        .map(|v| (v.label.clone(), v.display.clone()))
        .collect();
    let (headline, fallback) = match draft.kind {
        Kind::Decide => (
            format!(
                "*Decide this {}?* {}",
                noun(&draft.request.schema),
                text::escape(
                    &draft
                        .target
                        .as_ref()
                        .map(|t| t.summary.clone())
                        .unwrap_or_default()
                )
            ),
            "Decide this claim?",
        ),
        _ => (
            format!("*File this in {}?*", text::escape(&draft.title)),
            "File this record?",
        ),
    };
    let names: Vec<String> = draft
        .approvers
        .iter()
        .map(|a| format!("*{}*", text::escape(&a.label)))
        .collect();
    let approver = match (&draft.kind, &draft.approver) {
        (Kind::Create, Some(_)) if names.len() > 1 => {
            format!("Approvers: {}", names.join(", then "))
        }
        (Kind::Create, Some(approver)) => format!("Approver: *{}*", text::escape(&approver.label)),
        (Kind::Create, None) => "No approver named".to_owned(),
        _ => String::new(),
    };
    let mut blocks = vec![text::section(&headline), text::fields(&pairs)];
    // APP-107: the declared rule behind the status and the PO match, display only.
    let mut notes: Vec<String> = draft.rule.iter().map(|r| text::escape(r)).collect();
    notes.extend(
        draft
            .flags
            .iter()
            .filter_map(|f| f.get("label").and_then(Value::as_str))
            .map(text::escape),
    );
    if !notes.is_empty() {
        blocks.push(text::context(&notes.join(" · ")));
    }
    let mut note = String::from(
        "Nothing is written until you confirm. The command goes through Forma's audited path.",
    );
    if !approver.is_empty() {
        note = format!("{approver} · {note}");
    }
    blocks.push(text::context(&note));
    blocks.push(text::actions(
        "forma-draft",
        vec![
            text::button("Confirm", super::flows::CONFIRM, id, Some("primary")),
            text::button("Cancel", super::flows::CANCEL, id, None),
        ],
    ));
    let _ = state;
    text::ephemeral(fallback, Some(blocks), false)
}

/// A record as a short line: its text and money values, escaped.
fn summary(record: &RecordRow) -> String {
    text::escape(&resolve::summarize(record))
}

/// The member's writable works of record apps that declare a decision, and
/// their listings (at most `MAX_WORKS`).
async fn decidable_listings(
    stores: &storage::Stores,
    member: &TrustedRequest,
    catalog: &[CatalogApp],
) -> Result<Vec<storage::RecordsListing>, storage::Error> {
    let works = stores.data.openable_works(member).await?;
    let mut listings = Vec::new();
    for work in works
        .into_iter()
        .filter(|w| w.can_write)
        .filter(|w| {
            catalog
                .iter()
                .any(|c| c.app == w.app && c.decide_command().is_some())
        })
        .take(MAX_WORKS)
    {
        match stores
            .data
            .list_records(member, &work.app, &work.work)
            .await
        {
            Ok(listing) => listings.push(listing),
            Err(storage::Error::Denied) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(listings)
}

/// The record's decision buttons (the value is the record id alone).
fn decision_buttons(record: &RecordRow) -> Value {
    text::actions(
        &format!("decide-{}", record.record_id),
        vec![
            text::button("Approve", APPROVE, &record.record_id, Some("primary")),
            text::button("Reject", REJECT, &record.record_id, Some("danger")),
        ],
    )
}

/// What a record is called in a message: "claim" for an expense claim,
/// else its schema in words ("purchase request").
pub(crate) fn noun(schema: &str) -> String {
    if schema == "expense_claim" {
        "claim".into()
    } else {
        schema.replace('_', " ")
    }
}

/// The record as an approver reads it: its values (an expense claim's in
/// the claim's order), its status aside, and the filer.
fn claim_blocks(record: &RecordRow) -> Vec<Value> {
    let mut pairs = Vec::new();
    let expense = [
        ("amount", "Amount"),
        ("merchant", "Merchant"),
        ("spent_on", "Date"),
        ("category", "Category"),
        ("note", "Note"),
    ];
    if record.schema == "expense_claim" {
        for (field, label) in expense {
            if let Some(display) = resolve::record_display(record, field) {
                pairs.push((label.to_owned(), display));
            }
        }
    } else {
        for item in record.values.as_array().into_iter().flatten().take(8) {
            let (Some(field), Some(value)) = (
                item.get("field_id").and_then(Value::as_str),
                item.get("value"),
            ) else {
                continue;
            };
            if field == "status" || value.get("type").and_then(Value::as_str) == Some("null") {
                continue;
            }
            let mut label = field.replace('_', " ");
            if let Some(first) = label.get(0..1) {
                let upper = first.to_uppercase();
                label.replace_range(0..1, &upper);
            }
            pairs.push((label, resolve::display(value)));
        }
    }
    pairs.push(("Filed by".to_owned(), directory::label(&record.creator)));
    if pairs.len() == 1 {
        return vec![text::section(&summary(record)), text::fields(&pairs)];
    }
    vec![text::fields(&pairs)]
}

/// `open`: what is waiting for this member's decision.
async fn pending_message(state: &Slack, member: &TrustedRequest, catalog: &[CatalogApp]) -> Value {
    let Some(stores) = state.auth.stores.as_ref() else {
        return text::ephemeral(UNAVAILABLE, None, false);
    };
    let listings = match decidable_listings(stores, member, catalog).await {
        Ok(listings) => listings,
        Err(_) => return text::ephemeral(UNAVAILABLE, None, false),
    };
    let mut blocks = Vec::new();
    let mut count = 0;
    for listing in &listings {
        for record in resolve::pending(listing, member.principal()) {
            count += 1;
            if count > MAX_PENDING {
                continue;
            }
            blocks.extend(claim_blocks(record));
            blocks.push(decision_buttons(record));
        }
    }
    if count == 0 {
        return text::ephemeral(
            &format!(
                "Nothing is waiting for your approval. {}",
                canvas_link(state)
            ),
            None,
            false,
        );
    }
    let headline = format!(
        "*{count} waiting for your approval*{}",
        if count > MAX_PENDING {
            format!(" (showing {MAX_PENDING})")
        } else {
            String::new()
        }
    );
    blocks.insert(0, text::section(&headline));
    blocks.push(text::context(&canvas_link(state)));
    text::ephemeral(
        &format!("{count} waiting for your approval"),
        Some(blocks),
        false,
    )
}

/// `/forma <utterance>`: the action path as the linked member.
async fn act(
    state: Arc<Slack>,
    team: String,
    user: String,
    utterance: String,
    response_url: String,
) {
    let started = Instant::now();
    let message = act_message(&state, &team, &user, &utterance).await;
    tracing::info!(
        event = "slack.act",
        duration_ms = elapsed_ms(started),
        "slack utterance answered"
    );
    respond(&state, &response_url, message).await;
}

async fn act_message(state: &Slack, team: &str, user: &str, utterance: &str) -> Value {
    let Some(member) = member(state, team, user).await else {
        return text::ephemeral(LINK_PROMPT, None, false);
    };
    let Some(stores) = state.auth.stores.as_ref() else {
        return text::ephemeral(UNAVAILABLE, None, false);
    };
    let key = format!("act:{}:{}", member.organization(), member.principal());
    if state.auth.limits.admit(&[(&key, ACT_MEMBER)]).is_err() {
        return text::ephemeral(
            "You're going fast. Try again in a few minutes.",
            None,
            false,
        );
    }
    // APP-108: a folder named in the words ("build a metrics app from the
    // Q3 metrics folder", `/forma folder 1 formation brief me`); Slack has no
    // selected folder, so the name or number is required.
    if let Some(answer) = crate::boxes::intent::act(&member, utterance, None).await {
        return folder_message(state, answer);
    }
    let context = match stores.workspace.composer_context(&member).await {
        Ok(context) => context,
        Err(storage::Error::Denied) => return text::ephemeral(REFUSED, None, false),
        Err(_) => return text::ephemeral(UNAVAILABLE, None, false),
    };
    let catalog = prompt::catalog(&context.library);
    if catalog.is_empty() {
        return text::ephemeral(REFUSED, None, false);
    }
    let demo = demo::is_demo_org(member.organization());
    let today = crate::events::today();
    let parse = match state.parser.parse(utterance, &catalog, demo, &today).await {
        Ok(parse) => parse,
        Err(ParseFailure::Unavailable) => {
            return text::ephemeral("Forma's assistant isn't available right now.", None, false)
        }
        Err(ParseFailure::CouldNotParse(code)) => {
            tracing::info!(event = "slack.act_refused", code, "utterance not parsed");
            return text::ephemeral(
                &format!("Forma couldn't understand that.\n{HELP}"),
                None,
                false,
            );
        }
    };
    tracing::info!(
        event = "slack.parsed",
        organization_id = member.organization(),
        principal_id = member.principal(),
        kind = parse.parsed.kind.as_str(),
        rehearsed = parse.rehearsed,
        duration_ms = parse.duration_ms,
        "slack utterance parsed"
    );
    match parse.parsed.kind {
        Kind::Place => text::ephemeral(
            &format!("That one belongs on the canvas: {}", canvas_link(state)),
            None,
            false,
        ),
        Kind::Open => pending_message(state, &member, &catalog).await,
        Kind::Create | Kind::Decide => {
            match resolve::resolve(
                stores,
                &member,
                &context.library,
                &catalog,
                &parse.parsed,
                utterance,
                &today,
            )
            .await
            {
                Ok(Resolution::Draft(draft)) => {
                    let draft = *draft;
                    match state.hold(&member, team, user, draft.clone()) {
                        Some(id) => draft_card(state, &draft, &id),
                        None => text::ephemeral(UNAVAILABLE, None, false),
                    }
                }
                Ok(Resolution::Open { .. }) => pending_message(state, &member, &catalog).await,
                Ok(Resolution::Place) => text::ephemeral(
                    &format!("That one belongs on the canvas: {}", canvas_link(state)),
                    None,
                    false,
                ),
                Err(Refusal::NotFound) => text::ephemeral(REFUSED, None, false),
                Err(Refusal::StoreUnavailable) => text::ephemeral(UNAVAILABLE, None, false),
                Err(Refusal::CouldNotDraft(code)) => {
                    tracing::info!(event = "slack.act_refused", code, "could not draft");
                    text::ephemeral(
                        "Forma couldn't turn that into a record. Try saying the amount, the merchant and who approves.",
                        None,
                        false,
                    )
                }
            }
        }
    }
}

/// `block_actions` after verification: dispatch on `action_id` (spawned).
pub fn interaction(state: &Arc<Slack>, payload: &Value) {
    if payload.get("type").and_then(Value::as_str) != Some("block_actions") {
        return;
    }
    let text_at = |pointer: &str| {
        payload
            .pointer(pointer)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned()
    };
    let team = {
        let team = text_at("/team/id");
        if team.is_empty() {
            text_at("/user/team_id")
        } else {
            team
        }
    };
    let user = text_at("/user/id");
    let action = text_at("/actions/0/action_id");
    let value = text_at("/actions/0/value");
    let response_url = text_at("/response_url");
    if !(storage::slack_id(&team)
        && storage::slack_id(&user)
        && state.api.allowed_response_url(&response_url))
        || value.len() > 64
    {
        return;
    }
    let state = state.clone();
    match action.as_str() {
        CONFIRM => {
            tokio::spawn(async move { confirm(state, team, user, value, response_url).await });
        }
        CANCEL => {
            tokio::spawn(async move { cancel(state, team, user, value, response_url).await });
        }
        APPROVE | REJECT => {
            let to = if action == APPROVE {
                "approved"
            } else {
                "rejected"
            };
            tokio::spawn(async move { decide(state, team, user, value, to, response_url).await });
        }
        _ => {}
    }
}

/// The held draft `id` when it belongs to this member and Slack user.
fn held(state: &Slack, member: &TrustedRequest, team: &str, user: &str, id: &str) -> Option<Draft> {
    let drafts = state.drafts.lock().ok()?;
    let held = drafts.get(id)?;
    (held.created.elapsed() < super::DRAFT_TTL
        && held.organization == member.organization()
        && held.principal == member.principal()
        && held.team == team
        && held.user == user)
        .then(|| held.draft.clone())
}

async fn cancel(state: Arc<Slack>, team: String, user: String, id: String, response_url: String) {
    let Some(member) = member(&state, &team, &user).await else {
        return respond(
            &state,
            &response_url,
            text::ephemeral(LINK_PROMPT, None, false),
        )
        .await;
    };
    if held(&state, &member, &team, &user, &id).is_some() {
        if let Ok(mut drafts) = state.drafts.lock() {
            drafts.remove(&id);
        }
    }
    respond(
        &state,
        &response_url,
        text::ephemeral("Cancelled. Nothing was filed.", None, true),
    )
    .await;
}

/// Confirm: the held draft's command through the audited route, then the
/// share to the approver (a create) or the filer's DM (a decision).
async fn confirm(state: Arc<Slack>, team: String, user: String, id: String, response_url: String) {
    let started = Instant::now();
    let Some(member) = member(&state, &team, &user).await else {
        return respond(
            &state,
            &response_url,
            text::ephemeral(LINK_PROMPT, None, false),
        )
        .await;
    };
    let Some(draft) = held(&state, &member, &team, &user, &id) else {
        return respond(
            &state,
            &response_url,
            text::ephemeral(
                "This confirmation has expired. Run `/forma` again.",
                None,
                false,
            ),
        )
        .await;
    };
    let Some(stores) = state.auth.stores.as_ref() else {
        return respond(
            &state,
            &response_url,
            text::ephemeral(UNAVAILABLE, None, false),
        )
        .await;
    };
    let result = commit(stores, &member, &draft.request).await;
    let outcome = match &result {
        Ok(CommandResult::Committed(outcome)) | Ok(CommandResult::Replayed(outcome)) => {
            outcome.clone()
        }
        Ok(CommandResult::Conflict(_)) | Ok(CommandResult::Stale(_)) => {
            return respond(
                &state,
                &response_url,
                text::ephemeral(
                    "This changed since the draft. Run `/forma` again.",
                    None,
                    true,
                ),
            )
            .await;
        }
        Ok(CommandResult::InFlight) | Ok(CommandResult::Busy) => {
            return respond(
                &state,
                &response_url,
                text::ephemeral(
                    "Forma is still working on this. Press Confirm again in a moment.",
                    None,
                    false,
                ),
            )
            .await;
        }
        Err(storage::Error::Denied) => {
            return respond(&state, &response_url, text::ephemeral(REFUSED, None, false)).await;
        }
        Err(_) => {
            return respond(
                &state,
                &response_url,
                text::ephemeral(UNAVAILABLE, None, false),
            )
            .await
        }
    };
    let fresh = matches!(result, Ok(CommandResult::Committed(_)));
    if fresh {
        origins::mark(
            &outcome.record_id,
            outcome.record_version,
            super::flows::VIA,
        );
    }
    let shown: String = text::escape(
        &draft
            .values
            .iter()
            .filter(|v| v.field != "status" && v.field != "category" && v.field != "spent_on")
            .map(|v| v.display.clone())
            .filter(|d| d != "—")
            .collect::<Vec<_>>()
            .join(" · "),
    );
    let message = match draft.kind {
        Kind::Create => {
            let mut line = format!("Filed: *{shown}*");
            let mut sent = Vec::new();
            if let Some(rule) = &draft.rule {
                line.push_str(&format!(" ({})", text::escape(rule)));
            }
            // APP-107: every approver named (a two-level workflow's second too).
            for approver in &draft.approvers {
                match stores
                    .workspace
                    .share_work(&member, &draft.app, &draft.work, &approver.principal, true)
                    .await
                {
                    Ok(shared) => {
                        if shared.changed() {
                            events::hub().app_shared(
                                member.organization(),
                                &draft.app,
                                &draft.work,
                            );
                        }
                        sent.push(format!("*{}*", text::escape(&approver.label)));
                    }
                    Err(error) => {
                        tracing::warn!(event = "slack.share_failed", error = %error, "share to the approver failed");
                    }
                }
            }
            if !sent.is_empty() {
                line.push_str(&format!(", sent to {} for approval", sent.join(" and ")));
            }
            format!("{line}. {}", canvas_link(&state))
        }
        _ => {
            if fresh {
                notify_filer(&state, &member, &outcome, &draft).await;
            }
            let status = draft
                .values
                .iter()
                .find(|v| v.field == "status")
                .map(|v| v.display.clone())
                .unwrap_or_else(|| "decided".into());
            format!(
                "{} {}: *{}*. {}",
                capitalized(&status),
                time_now(),
                text::escape(
                    &draft
                        .target
                        .as_ref()
                        .map(|t| t.summary.clone())
                        .unwrap_or_default()
                ),
                canvas_link(&state)
            )
        }
    };
    tracing::info!(
        event = "slack.confirmed",
        organization_id = member.organization(),
        principal_id = member.principal(),
        kind = draft.kind.as_str(),
        replayed = !fresh,
        duration_ms = elapsed_ms(started),
        "slack draft confirmed"
    );
    respond(&state, &response_url, text::ephemeral(&message, None, true)).await;
}

fn capitalized(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}
fn time_now() -> String {
    format!("· {}", text::time(super::now()))
}

/// The filer's DM after a decision (when the filer is linked and is not the
/// decider).
async fn notify_filer_record(
    state: &Slack,
    decider: &TrustedRequest,
    record: &RecordRow,
    status: &str,
) {
    if record.creator == decider.principal() {
        return;
    }
    let Some(stores) = state.auth.stores.as_ref() else {
        return;
    };
    let Ok(links) = stores.workspace.slack_links(decider.organization()).await else {
        return;
    };
    let what = {
        let merchant = resolve::record_display(record, "merchant");
        let amount = resolve::record_display(record, "amount");
        match (merchant, amount) {
            (Some(m), Some(a)) => format!("{} claim ({})", text::escape(&m), text::escape(&a)),
            _ => format!("{} ({})", noun(&record.schema), summary(record)),
        }
    };
    let text_line = format!(
        "Your {what} was *{}* by *{}*.",
        text::escape(status),
        text::escape(&directory::label(decider.principal()))
    );
    for (team, user, principal) in links.into_iter().filter(|(_, _, p)| *p == record.creator) {
        // The filer's link is judged again before anything is sent to it.
        match stores.workspace.resolve_slack_link(&team, &user).await {
            Ok(Some(filer))
                if filer.principal() == principal
                    && filer.organization() == decider.organization() =>
            {
                if let Err(error) = state.api.direct(&user, &text_line, None).await {
                    tracing::warn!(
                        event = "slack.dm_failed",
                        code = error.code(),
                        "filer DM not sent"
                    );
                } else {
                    tracing::info!(
                        event = "slack.filer_notified",
                        organization_id = decider.organization(),
                        "filer notified"
                    );
                }
            }
            _ => {}
        }
    }
}

async fn notify_filer(
    state: &Slack,
    member: &TrustedRequest,
    outcome: &CommandOutcome,
    draft: &Draft,
) {
    let Some(stores) = state.auth.stores.as_ref() else {
        return;
    };
    let Ok(listing) = stores
        .data
        .list_records(member, &outcome.app, &outcome.work)
        .await
    else {
        return;
    };
    let status = draft
        .values
        .iter()
        .find(|v| v.field == "status")
        .map(|v| v.display.clone())
        .unwrap_or_else(|| "decided".into());
    if let Some(record) = listing
        .records
        .iter()
        .find(|r| r.record_id == outcome.record_id)
    {
        notify_filer_record(state, member, record, &status).await;
    }
}

/// Approve / Reject on a record: judged as the clicking member, through the
/// audited route; the message loses its buttons; the filer is told.
async fn decide(
    state: Arc<Slack>,
    team: String,
    user: String,
    record_id: String,
    to: &'static str,
    response_url: String,
) {
    let started = Instant::now();
    let Some(member) = member(&state, &team, &user).await else {
        return respond(
            &state,
            &response_url,
            text::ephemeral(LINK_PROMPT, None, false),
        )
        .await;
    };
    let refuse = |state: Arc<Slack>, message: &'static str| {
        let response_url = response_url.clone();
        async move { respond(&state, &response_url, text::ephemeral(message, None, false)).await }
    };
    if !storage::valid_identifier(&record_id) {
        return refuse(state, REFUSED).await;
    }
    let Some(stores) = state.auth.stores.as_ref() else {
        return refuse(state.clone(), UNAVAILABLE).await;
    };
    let catalog = match stores.workspace.composer_context(&member).await {
        Ok(context) => prompt::catalog(&context.library),
        Err(storage::Error::Denied) => return refuse(state.clone(), REFUSED).await,
        Err(_) => return refuse(state.clone(), UNAVAILABLE).await,
    };
    let listings = match decidable_listings(stores, &member, &catalog).await {
        Ok(listings) => listings,
        Err(_) => return refuse(state.clone(), UNAVAILABLE).await,
    };
    let mut found = None;
    for listing in &listings {
        match resolve::exact_decision(listing, &record_id, to) {
            Err(Refusal::NotFound) => continue,
            other => {
                found = Some(other);
                break;
            }
        }
    }
    let (request, record) = match found {
        Some(Ok((request, record))) => (request, record.clone()),
        Some(Err(Refusal::CouldNotDraft("already_decided"))) => {
            return refuse(state.clone(), "This claim was already decided.").await;
        }
        Some(Err(Refusal::StoreUnavailable)) => return refuse(state.clone(), UNAVAILABLE).await,
        _ => {
            tracing::info!(
                event = "slack.decide_refused",
                organization_id = member.organization(),
                principal_id = member.principal(),
                "decision refused"
            );
            return refuse(state.clone(), REFUSED).await;
        }
    };
    // APP-107: the status actually drafted (a guarded step may be
    // "manager approved" where the button said Approve).
    let actual = request
        .values
        .get(0)
        .and_then(|v| v.pointer("/value/value"))
        .and_then(Value::as_str)
        .map(|v| v.replace('_', " "))
        .unwrap_or_else(|| to.to_owned());
    let to = actual.as_str();
    let result = commit(stores, &member, &request).await;
    let message = match result {
        Ok(CommandResult::Committed(outcome)) => {
            origins::mark(&outcome.record_id, outcome.record_version, VIA);
            notify_filer_record(&state, &member, &record, to).await;
            let mut blocks = claim_blocks(&record);
            blocks.insert(
                0,
                text::section(&format!(
                    "*{} by {}* {}",
                    capitalized(to),
                    text::escape(&directory::label(member.principal())),
                    time_now()
                )),
            );
            blocks.push(text::context(&canvas_link(&state)));
            text::ephemeral(
                &format!(
                    "{} by {}",
                    capitalized(to),
                    text::escape(&directory::label(member.principal()))
                ),
                Some(blocks),
                true,
            )
        }
        Ok(CommandResult::Conflict(_)) | Ok(CommandResult::Stale(_)) => {
            text::ephemeral("This claim was already decided.", None, false)
        }
        Ok(_) => text::ephemeral(
            "Forma is still working on this. Try again in a moment.",
            None,
            false,
        ),
        Err(storage::Error::Denied) => text::ephemeral(REFUSED, None, false),
        Err(_) => text::ephemeral(UNAVAILABLE, None, false),
    };
    tracing::info!(
        event = "slack.decided",
        organization_id = member.organization(),
        principal_id = member.principal(),
        to,
        duration_ms = elapsed_ms(started),
        "slack decision handled"
    );
    respond(&state, &response_url, message).await;
}

/// `event_callback` after verification: `app_mention` answers the help text
/// in the mention's thread.
pub fn event(state: &Arc<Slack>, body: &Value) {
    let event = body.get("event");
    if event.and_then(|e| e.get("type")).and_then(Value::as_str) != Some("app_mention") {
        return;
    }
    let channel = event
        .and_then(|e| e.get("channel"))
        .and_then(Value::as_str)
        .filter(|c| storage::slack_id(c))
        .map(str::to_owned);
    let thread = event
        .and_then(|e| e.get("ts"))
        .and_then(Value::as_str)
        .filter(|ts| ts.len() <= 32 && ts.bytes().all(|b| b.is_ascii_digit() || b == b'.'))
        .map(str::to_owned);
    let Some(channel) = channel else {
        return;
    };
    let state = state.clone();
    tokio::spawn(async move {
        let mut body = serde_json::json!({"channel": channel, "text": HELP, "unfurl_links": false});
        if let Some(thread) = thread {
            body["thread_ts"] = Value::String(thread);
        }
        if let Err(error) = state.api.call("chat.postMessage", &body).await {
            tracing::warn!(
                event = "slack.mention_failed",
                code = error.code(),
                "mention reply not sent"
            );
        }
    });
}

/// The approver's DM for a pending claim (used by `notify`).
pub(crate) fn approval_request(record: &RecordRow, state: &Slack) -> (String, Vec<Value>) {
    let filer = text::escape(&directory::label(&record.creator));
    let what = noun(&record.schema);
    let mut blocks = vec![text::section(&format!(
        "*{filer}* filed a {what} for your approval"
    ))];
    blocks.extend(claim_blocks(record));
    blocks.push(decision_buttons(record));
    blocks.push(text::context(&canvas_link(state)));
    (format!("{filer} filed a {what} for your approval"), blocks)
}

/// APP-108: a folder answer as an ephemeral Slack message (text only; every
/// value is escaped; numbers come from receipts or page citations).
fn folder_message(state: &Slack, answer: crate::boxes::intent::Answer) -> Value {
    use crate::boxes::intent::Answer;
    let noun = crate::boxes::NOUN_LOWER;
    let value = match answer {
        Answer::Done(value) => value,
        Answer::NotAFolderRequest => return text::ephemeral(REFUSED, None, false),
        Answer::Refused(refused) => {
            let message = match refused {
                crate::boxes::routes::BuildRefusal::NoData => format!("That {noun} has no CSV or XLSX data to build from yet."),
                crate::boxes::routes::BuildRefusal::ComposerUnavailable => UNAVAILABLE.to_owned(),
                crate::boxes::routes::BuildRefusal::TooMany(_) => "You're going fast. Try again in a few minutes.".to_owned(),
                crate::boxes::routes::BuildRefusal::CouldNotCompose(_) => format!("Forma could not build an app from that {noun}. Try saying what it should show."),
                _ => REFUSED.to_owned(),
            };
            return text::ephemeral(&message, None, false);
        }
    };
    let parts = |parts: &Value| -> String {
        parts
            .as_array()
            .into_iter()
            .flatten()
            .map(|p| {
                if let Some(v) = p.get("value").and_then(Value::as_str) {
                    v.to_owned()
                } else if let Some(page) = p
                    .get("cite")
                    .and_then(|c| c.get("page"))
                    .and_then(Value::as_u64)
                {
                    format!(
                        "{} (p. {page})",
                        p.get("text").and_then(Value::as_str).unwrap_or("").trim()
                    )
                } else {
                    p.get("text")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("")
    };
    match value["kind"].as_str() {
        Some("build_from_box") => {
            let url = format!(
                "{}/w/{}/{}",
                state.origin(),
                value["app_id"].as_str().unwrap_or(""),
                value["work_id"].as_str().unwrap_or("")
            );
            let line = format!(
                "{} is on your canvas ({}). {}",
                text::escape(value["title"].as_str().unwrap_or("Your app")),
                text::escape(value["label"].as_str().unwrap_or("")),
                text::link(&url, "Open it")
            );
            text::ephemeral(
                &line,
                Some(vec![
                    text::section(&line),
                    text::context(&canvas_link(state)),
                ]),
                false,
            )
        }
        Some("brief") => {
            let brief = &value["brief"];
            let mut lines = vec![text::escape(&parts(&brief["overview"]))];
            for file in brief["files"].as_array().into_iter().flatten() {
                let body = match file["status"].as_str() {
                    Some("too_large") => "too large for the brief in this plan".to_owned(),
                    _ => parts(&file["paragraph"]),
                };
                if !body.is_empty() {
                    lines.push(format!(
                        "*{}*: {}",
                        text::escape(file["name"].as_str().unwrap_or("")),
                        text::escape(&body)
                    ));
                }
            }
            let message = lines.join("\n");
            text::ephemeral(
                &message,
                Some(vec![
                    text::section(&message),
                    text::context("AI brief · cited from your files"),
                ]),
                false,
            )
        }
        Some("choose_folder") => {
            let choices: Vec<String> = value["choices"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|c| {
                    format!(
                        "{}. {}",
                        c["number"],
                        text::escape(c["name"].as_str().unwrap_or(""))
                    )
                })
                .collect();
            let message = format!("Which {noun}? {}. Say it by number, e.g. `/forma {noun} 1 formation build a metrics app`.", choices.join(", "));
            text::ephemeral(&message, None, false)
        }
        _ => text::ephemeral(REFUSED, None, false),
    }
}
