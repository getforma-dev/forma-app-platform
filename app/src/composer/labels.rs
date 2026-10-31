//! APP-121 naming: the label route and the effective names every surface
//! reads (`GET /v1/commands`, MCP `forma_list_apps`, the confirm card's
//! draft title, Slack's card).
//!
//! `POST /v1/apps/{app}/labels` changes one label:
//!
//! - `{"scope":"organization","card":"","name","address","aliases","description"}`:
//!   the app's display name, address (`/spend`), kept aliases and
//!   description; with a `card` (`ledger`), that card's display name and
//!   description. The app's owner or an admin only (`Workspace::label_app`);
//!   audited `app.labeled`.
//! - `{"scope":"personal","card","name","aliases"}`: the member's own name
//!   for a card and own shortcuts (`/exp`, `/q3`), on an app they can open
//!   (`Workspace::label_mine`); audited `app.aliased`.
//!
//! Every key but `scope` is optional: an absent key keeps the stored value,
//! `null` or `""` clears it, so the catalog's name shows again. Names are
//! case-insensitive (lowercased) addresses `[a-z][a-z0-9-]{1,31}`; no address,
//! alias or shortcut takes a verb ([`VERBS`]) or another app's address or
//! alias, and renaming an address keeps the old one as an alias. A personal
//! shortcut that an organization name later takes is dropped when read (the
//! organization's address wins).
//!
//! Refusals: the uniform 404 for no session, an app of another organization,
//! an app the member cannot open and (organization scope) a member who is
//! neither its owner nor an admin; 403 `cross_site`; 413; 422
//! `invalid_label`, `invalid_name`, `verb_name` or `name_taken`; 503.
use crate::{
    auth::{csrf, Auth},
    http::{self, UniformPath},
    storage::{
        self, address_shaped, card_shaped, label_description, label_name, Error, LabelChange,
        Labeled, Labels, NameRules,
    },
};
use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Router,
};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

pub const LABELS_PATH: &str = "/v1/apps/{app}/labels";
const BODY_LIMIT: usize = 4096;

/// The canvas verbs (`command-registry.ts`: the sixteen with APP-122's
/// `/shape` and `/ask`, their hidden
/// aliases) and the reserved list names: no address, alias or shortcut
/// may take one.
pub const VERBS: &[&str] = &[
    "app",
    "card",
    "widget",
    "arrange",
    "unite",
    "split",
    "focus",
    "folder",
    "rename",
    "alias",
    "shape",
    "ask",
    "apps",
    "commands",
    "shortcuts",
    "agent",
    "weather",
    "calculator",
    "clock",
    "note",
    "help",
    "new",
];

pub fn router(auth: Arc<Auth>) -> Router {
    Router::new()
        .route(LABELS_PATH, post(label))
        .with_state(auth)
}

/// Every library app's catalog address and aliases, by app id.
pub fn catalog_names() -> &'static BTreeMap<String, Vec<String>> {
    static NAMES: OnceLock<BTreeMap<String, Vec<String>>> = OnceLock::new();
    NAMES.get_or_init(|| {
        storage::library_sidecar()
            .unwrap_or_default()
            .iter()
            .filter_map(|entry| {
                let app = entry.get("app_id")?.as_str()?.to_owned();
                let command = entry.get("command")?;
                let mut names = vec![command.get("slug")?.as_str()?.to_owned()];
                names.extend(
                    command
                        .get("aliases")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .map(str::to_owned),
                );
                Some((app, names))
            })
            .collect()
    })
}

/// The rules a label write of `app` is judged by: its own catalog names
/// (kept forever), every other app's, and the verbs.
pub fn rules_for(app: &str) -> NameRules {
    let names = catalog_names();
    let others = || names.iter().filter(|(id, _)| id.as_str() != app);
    NameRules {
        own: names.get(app).cloned().unwrap_or_default(),
        others: others()
            .filter_map(|(_, list)| list.first().cloned())
            .collect(),
        other_aliases: others()
            .flat_map(|(_, list)| list.iter().skip(1).cloned())
            .collect(),
        verbs: VERBS.iter().map(|verb| (*verb).to_owned()).collect(),
    }
}

/// One card's labels as one member sees them.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CardNaming {
    pub card: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub my_name: Option<String>,
    pub mine: Vec<String>,
}

/// One app's labels as one member sees them; `None` fields fall back to the catalog.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Naming {
    pub name: Option<String>,
    pub description: Option<String>,
    pub address: Option<String>,
    /// The organization's kept aliases (beyond the catalog's).
    pub aliases: Vec<String>,
    /// The member's own shortcuts that no organization name takes.
    pub mine: Vec<String>,
    pub cards: Vec<CardNaming>,
    pub can_label: bool,
    /// Names other apps' organization labels took (a catalog alias of this
    /// app among them no longer answers for it).
    pub claimed: BTreeSet<String>,
}

/// Every organization name: the catalog's and the labels' addresses and aliases.
fn organization_names(labels: &Labels) -> BTreeSet<String> {
    catalog_names()
        .values()
        .flatten()
        .cloned()
        .chain(
            labels
                .organization
                .iter()
                .flat_map(|l| l.address.iter().chain(&l.aliases).cloned()),
        )
        .collect()
}

/// `app`'s naming for this member (module notes).
pub fn naming(labels: &Labels, app: &str) -> Naming {
    let taken = organization_names(labels);
    let free = |list: &[String]| -> Vec<String> {
        list.iter()
            .filter(|alias| !taken.contains(*alias))
            .cloned()
            .collect()
    };
    let mut naming = Naming {
        can_label: labels.labelable.contains(app),
        claimed: labels
            .organization
            .iter()
            .filter(|l| l.app != app)
            .flat_map(|l| l.address.iter().chain(&l.aliases).cloned())
            .collect(),
        ..Naming::default()
    };
    for label in labels.organization.iter().filter(|l| l.app == app) {
        if label.card.is_empty() {
            naming.name = label.name.clone();
            naming.description = label.description.clone();
            naming.address = label.address.clone();
            naming.aliases = label.aliases.clone();
        } else {
            naming.cards.push(CardNaming {
                card: label.card.clone(),
                name: label.name.clone(),
                description: label.description.clone(),
                ..CardNaming::default()
            });
        }
    }
    for mine in labels.mine.iter().filter(|l| l.app == app) {
        if mine.card.is_empty() {
            naming.mine = free(&mine.aliases);
            continue;
        }
        let at = match naming.cards.iter().position(|c| c.card == mine.card) {
            Some(at) => at,
            None => {
                naming.cards.push(CardNaming {
                    card: mine.card.clone(),
                    ..CardNaming::default()
                });
                naming.cards.len() - 1
            }
        };
        naming.cards[at].my_name = mine.name.clone();
        naming.cards[at].mine = free(&mine.aliases);
    }
    naming.cards.sort_by(|a, b| a.card.cmp(&b.card));
    naming
}

/// The address, aliases (catalog, organization, then the member's own) and
/// title an app is shown with, from its catalog slug, aliases and title.
pub fn effective(
    naming: &Naming,
    slug: &str,
    aliases: &[String],
    title: &str,
) -> (String, Vec<String>, String) {
    let address = naming.address.clone().unwrap_or_else(|| slug.to_owned());
    let mut seen = BTreeSet::from([address.clone()]);
    let all = std::iter::once(slug.to_owned())
        .chain(
            aliases
                .iter()
                .filter(|alias| !naming.claimed.contains(*alias))
                .cloned(),
        )
        .chain(naming.aliases.iter().cloned())
        .chain(naming.mine.iter().cloned())
        .filter(|name| seen.insert(name.clone()))
        .collect();
    (
        address,
        all,
        naming.name.clone().unwrap_or_else(|| title.to_owned()),
    )
}

/// `naming` as the JSON the canvas reads beside a `GET /v1/commands` entry.
pub fn naming_json(naming: &Naming) -> Map<String, Value> {
    let mut map = Map::new();
    map.insert(
        "description".into(),
        json!(naming.description.clone().unwrap_or_default()),
    );
    map.insert("org_aliases".into(), json!(naming.aliases));
    map.insert("mine".into(), json!(naming.mine));
    map.insert("can_label".into(), json!(naming.can_label));
    map.insert(
        "cards".into(),
        Value::Array(
            naming
                .cards
                .iter()
                .map(|card| {
                    json!({
                        "card": card.card,
                        "name": card.name,
                        "description": card.description,
                        "my_name": card.my_name,
                        "mine": card.mine,
                    })
                })
                .collect(),
        ),
    );
    map
}

/// The display title of `app` for this member: the organization's label, else `fallback`.
pub async fn title_of(
    stores: &storage::Stores,
    member: &storage::TrustedRequest,
    app: &str,
    fallback: String,
) -> String {
    match stores.workspace.labels(member).await {
        Ok(labels) => naming(&labels, app).name.unwrap_or(fallback),
        Err(_) => fallback,
    }
}

/// The parsed request body (module notes); `None` is `invalid_label`.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct Body {
    pub personal: bool,
    pub card: String,
    pub change: LabelChange,
}

fn optional_text(
    value: &Value,
    shape: fn(&str) -> Option<Option<String>>,
) -> Option<Option<String>> {
    match value {
        Value::Null => Some(None),
        Value::String(text) => shape(text),
        _ => None,
    }
}

/// A list of addresses (`["/Spend", "team-spend"]`), each shaped; `None` refuses all.
pub fn names(value: &Value) -> Option<Vec<String>> {
    match value {
        Value::Null => Some(Vec::new()),
        Value::Array(list) if list.len() <= storage::MAX_LABEL_ALIASES => list
            .iter()
            .map(|item| item.as_str().and_then(address_shaped))
            .collect(),
        _ => None,
    }
}

/// Parses a label body. `Err(code)` names the 422 problem.
pub fn parse_body(bytes: &[u8]) -> Result<Body, &'static str> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| "invalid_label")?;
    let object = value.as_object().ok_or("invalid_label")?;
    let personal = match object.get("scope").and_then(Value::as_str) {
        Some("organization") => false,
        Some("personal") => true,
        _ => return Err("invalid_label"),
    };
    let card = match object.get("card") {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(card)) if card_shaped(card) => card.clone(),
        _ => return Err("invalid_label"),
    };
    let allowed: &[&str] = match (personal, card.is_empty()) {
        (true, _) => &["scope", "card", "name", "aliases"],
        (false, true) => &["scope", "card", "name", "description", "address", "aliases"],
        (false, false) => &["scope", "card", "name", "description"],
    };
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err("invalid_label");
    }
    let mut change = LabelChange::default();
    if let Some(name) = object.get("name") {
        change.name = Some(optional_text(name, label_name).ok_or("invalid_label")?);
    }
    if let Some(description) = object.get("description") {
        change.description =
            Some(optional_text(description, label_description).ok_or("invalid_label")?);
    }
    if let Some(address) = object.get("address") {
        change.address = Some(match address {
            Value::Null => None,
            Value::String(text) if text.trim().is_empty() => None,
            Value::String(text) => Some(address_shaped(text).ok_or("invalid_name")?),
            _ => return Err("invalid_label"),
        });
    }
    if let Some(aliases) = object.get("aliases") {
        change.aliases = Some(names(aliases).ok_or("invalid_name")?);
    }
    Ok(Body {
        personal,
        card,
        change,
    })
}

fn document(status: StatusCode, value: &Value) -> Response {
    match crate::admission::encode(value) {
        Ok(bytes) => {
            let mut response = (status, bytes).into_response();
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            http::secure(response.headers_mut());
            response
        }
        Err(_) => http::unavailable(),
    }
}

fn answer<T>(labeled: Result<Labeled<T>, Error>, view: impl Fn(&T) -> Value) -> Response {
    match labeled {
        Ok(Labeled::Saved(label)) => document(
            StatusCode::OK,
            &json!({"saved": true, "label": view(&label)}),
        ),
        Ok(Labeled::Unchanged(label)) => document(
            StatusCode::OK,
            &json!({"saved": false, "label": view(&label)}),
        ),
        Ok(Labeled::Refused(code)) => http::problem(StatusCode::UNPROCESSABLE_ENTITY, code),
        Err(Error::Denied) => http::not_found(),
        Err(_) => http::unavailable(),
    }
}

/// `POST /v1/apps/{app}/labels`.
async fn label(
    State(auth): State<Arc<Auth>>,
    UniformPath(app): UniformPath<String>,
    request: Request,
) -> Response {
    let (parts, body) = request.into_parts();
    if !csrf::same_origin(&parts.headers, auth.origin()) {
        return http::cross_site();
    }
    if !storage::valid_identifier(&app) || parts.uri.query().is_some() {
        return http::not_found();
    }
    let Some((_, member)) = crate::sharing::session(&auth, &parts.headers).await else {
        return http::not_found();
    };
    if let Err(retry_after) = auth
        .limits
        .admit_member_write(member.organization(), member.principal())
    {
        return http::too_many_requests(retry_after);
    }
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    let Some(bytes) = http::bounded(body, BODY_LIMIT).await else {
        return http::payload_too_large();
    };
    let body = match parse_body(&bytes) {
        Ok(body) => body,
        Err(code) => return http::problem(StatusCode::UNPROCESSABLE_ENTITY, code),
    };
    let rules = rules_for(&app);
    let response = if body.personal {
        let result = stores
            .workspace
            .label_mine(
                &member,
                &app,
                &body.card,
                body.change.name,
                body.change.aliases,
                &rules,
            )
            .await;
        answer(
            result,
            |label| json!({"app_id": label.app, "card": label.card, "name": label.name, "aliases": label.aliases}),
        )
    } else {
        let result = stores
            .workspace
            .label_app(&member, &app, &body.card, &body.change, &rules)
            .await;
        answer(
            result,
            |label| json!({"app_id": label.app, "card": label.card, "name": label.name, "description": label.description, "address": label.address, "aliases": label.aliases}),
        )
    };
    if response.status() == StatusCode::OK {
        tracing::info!(
            event = if body.personal {
                "app.aliased"
            } else {
                "app.labeled"
            },
            organization_id = member.organization(),
            principal_id = member.principal(),
            app_id = app,
            "label changed"
        );
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{Label, MemberLabel};

    const EXPENSES: &str = "00000000-0000-4000-8000-000000001681";

    #[test]
    fn bodies_are_closed_and_shaped() {
        let body =
            parse_body(br#"{"scope":"organization","name":"Team spend","address":"/Spend"}"#)
                .unwrap();
        assert!(!body.personal && body.card.is_empty());
        assert_eq!(body.change.name, Some(Some("Team spend".into())));
        assert_eq!(body.change.address, Some(Some("spend".into())));
        assert_eq!(
            body.change.aliases, None,
            "absent keys keep the stored value"
        );
        let card = parse_body(
            br#"{"scope":"organization","card":"total","name":"Q3 spend","description":null}"#,
        )
        .unwrap();
        assert_eq!(card.card, "total");
        assert_eq!(card.change.description, Some(None));
        // A card has no address; a personal label no description; no extra keys.
        for bad in [
            &br#"{"scope":"organization","card":"total","address":"q3"}"#[..],
            br#"{"scope":"personal","description":"x"}"#,
            br#"{"scope":"organization","name":"x","grants":["admin"]}"#,
            br#"{"scope":"everyone","name":"x"}"#,
            br#"{"scope":"organization","card":"Total"}"#,
            br#"{"scope":"organization","name":"a\u0007b"}"#,
        ] {
            assert_eq!(parse_body(bad).err(), Some("invalid_label"));
        }
        assert_eq!(
            parse_body(br#"{"scope":"personal","aliases":["/Exp","team spend"]}"#).err(),
            Some("invalid_name")
        );
        let mine = parse_body(br#"{"scope":"personal","aliases":["/EXP"]}"#).unwrap();
        assert_eq!(mine.change.aliases, Some(vec!["exp".to_owned()]));
    }

    #[test]
    fn rules_know_the_catalog_and_the_verbs() {
        let rules = rules_for(EXPENSES);
        assert_eq!(
            rules.own[..2],
            ["expenses".to_owned(), "expense".to_owned()]
        );
        assert!(!rules.others.contains("expenses") && rules.others.contains("time-off"));
        // Vendor spend's hidden /spend may become another app's address; its
        // address may not.
        assert!(
            rules.other_aliases.contains("spend")
                && rules.others.contains("vendor-spend-breakdown")
        );
        for verb in ["card", "app", "rename", "alias", "weather"] {
            assert!(rules.verbs.contains(verb), "{verb}");
        }
    }

    #[test]
    fn the_organization_address_wins_over_a_personal_alias() {
        let labels = Labels {
            organization: vec![
                Label {
                    app: EXPENSES.into(),
                    card: String::new(),
                    name: Some("Team spend".into()),
                    address: Some("spend".into()),
                    ..Label::default()
                },
                Label {
                    app: EXPENSES.into(),
                    card: "total".into(),
                    name: Some("Q3 spend".into()),
                    ..Label::default()
                },
            ],
            mine: vec![
                MemberLabel {
                    app: EXPENSES.into(),
                    card: String::new(),
                    name: None,
                    aliases: vec!["exp".into(), "spend".into(), "pto".into()],
                },
                MemberLabel {
                    app: EXPENSES.into(),
                    card: "total".into(),
                    name: None,
                    aliases: vec!["q3".into()],
                },
            ],
            labelable: BTreeSet::new(),
        };
        let naming = naming(&labels, EXPENSES);
        assert_eq!(
            naming.mine,
            ["exp"],
            "/spend and /pto are organization names"
        );
        assert!(!naming.can_label);
        assert_eq!(naming.cards[0].name.as_deref(), Some("Q3 spend"));
        assert_eq!(naming.cards[0].mine, ["q3"]);
        let (address, aliases, title) = effective(
            &naming,
            "expenses",
            &["expense".into(), "claim".into()],
            "Expenses",
        );
        assert_eq!(address, "spend");
        assert_eq!(aliases, ["expenses", "expense", "claim", "exp"]);
        assert_eq!(title, "Team spend");
        // Vendor spend no longer answers to the /spend Expenses took.
        let vendor = super::naming(&labels, "00000000-0000-4000-8000-000000001101");
        let (_, kept, _) = effective(
            &vendor,
            "vendor-spend-breakdown",
            &["vendor-spend".into(), "spend".into()],
            "Vendor spend",
        );
        assert_eq!(kept, ["vendor-spend"]);
        // No labels: everything falls back to the catalog.
        let plain = super::naming(&Labels::default(), EXPENSES);
        assert_eq!(
            effective(&plain, "expenses", &["expense".into()], "Expenses"),
            ("expenses".into(), vec!["expense".into()], "Expenses".into())
        );
    }
}
