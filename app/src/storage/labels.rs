//! APP-121 (ADR-011 pitch, `pivot/naming`): names people give apps and
//! cards (migration 0014).
//!
//! Two layers. **Organization labels** (`app_labels`): an app's display name,
//! address and description, and each card's display name and description
//! (the addresses an app had before a rename are kept automatically,
//! `kept`; there are no other organization aliases), written only by the
//! app's owner or a holder of `admin_policy` who can open the app
//! (`discover`), audited `app.labeled`.
//! **Personal labels** (`member_labels`): one member's own shortcuts (`/exp`
//! → an app, `/q3` → a card) and own card names, written by that member on
//! an app they can open, audited `app.aliased`, read back only for them.
//!
//! Labels never touch a definition, revision, publication, record or
//! receipt; without a row every surface falls back to the catalog's names.
//! Every operation is one READ COMMITTED workspace transaction under the
//! member fence (`fence_member`) and the request's RLS scope, so a foreign
//! organization's apps are unreachable and every refusal is `Denied` (the
//! route's uniform 404). Name rules are judged under a per-organization
//! advisory lock ([`lock_names`], also taken by "Save as /name" in
//! `create_work`), so two concurrent writes cannot both take one name; the
//! address is also unique in storage. A new organization address (and so a
//! kept one) never takes a name another app answers to: another app's
//! catalog address or alias (hidden or not), another label's current or
//! kept address, a saved work's `/name` or a verb or verb alias; an app's display name is never another app's name or
//! display title. Those names are read organization-wide, never narrowed to
//! what the member can open, and a clash is the same `name_taken` whether or
//! not the member can see the holder. Only a library app or an app shared
//! beyond its owner gets an address (a private app cannot squat a name). A personal shortcut never takes an organization name. A label
//! names only a card the app has; a member keeps at most
//! [`MAX_MEMBER_LABELS`] personal labels. A write whose result equals the
//! stored row writes nothing and appends no event (idempotent).
use super::{
    authorization::Transaction,
    definitions::{append, confirm_identity, Chain, Event},
    pool::Workspace,
    require, valid_id, Error, Result, TrustedRequest,
};
use std::collections::BTreeSet;
use std::time::Duration;
use tokio::time::Instant;
use tokio_postgres::Row;

/// ADR-007 D7.14 handler deadline for each transaction here.
pub const LABEL_DEADLINE: Duration = Duration::from_secs(5);
/// At most this many kept addresses (organization) or shortcuts (personal) per target.
pub const MAX_LABEL_ALIASES: usize = 8;
/// At most this many personal labels (rows) per member.
pub const MAX_MEMBER_LABELS: i64 = 100;
const NAME_CHARS: usize = 60;
const DESCRIPTION_CHARS: usize = 280;

/// An organization label of an app (`card` empty) or of one of its cards.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Label {
    pub app: String,
    pub card: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub address: Option<String>,
    /// The addresses the app had before a rename, kept automatically so
    /// nothing breaks (system-managed: never written directly; there are no
    /// other organization aliases).
    pub kept: Vec<String>,
}

/// One member's own label of an app or card: their name and shortcuts.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MemberLabel {
    pub app: String,
    pub card: String,
    pub name: Option<String>,
    pub aliases: Vec<String>,
}

/// What a member sees: the organization's labels of the apps they can open,
/// their own labels, and the apps whose organization labels they may change.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Labels {
    pub organization: Vec<Label>,
    pub mine: Vec<MemberLabel>,
    pub labelable: BTreeSet<String>,
}

/// A change: `None` keeps the field, `Some(None)` clears it, `Some(Some(v))`
/// sets it. `aliases` are a member's own shortcuts (personal scope only):
/// an organization label has no editable aliases.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LabelChange {
    pub name: Option<Option<String>>,
    pub description: Option<Option<String>>,
    pub address: Option<Option<String>>,
    pub aliases: Option<Vec<String>>,
}

/// The names a write is judged against that storage does not hold: the
/// catalog's address and aliases of the target app (`own`, always kept),
/// every name another catalog app answers to (`others`: its address and
/// every alias, hidden or not; catalog names always stay with their app),
/// the other catalog apps' display titles (`titles`, lowercased), the
/// reserved verbs and whether the target is a library app.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NameRules {
    pub own: Vec<String>,
    pub others: BTreeSet<String>,
    pub titles: BTreeSet<String>,
    pub verbs: BTreeSet<String>,
    pub library: bool,
}

/// The names storage holds that a write is judged against (organization
/// wide): every address and alias of the other apps' labels and every saved
/// work's `/name` (`names`), and the other apps' labeled display names,
/// lowercased (`titles`).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HeldNames {
    pub names: BTreeSet<String>,
    pub titles: BTreeSet<String>,
}

/// The outcome of a label write the member was allowed to make.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Labeled<T> {
    /// Written and audited.
    Saved(T),
    /// The stored label already said this; nothing was written.
    Unchanged(T),
    /// A rule refused it: `invalid_name`, `invalid_label`, `verb_name`,
    /// `name_taken`, `not_shared` or `too_many_labels`.
    Refused(&'static str),
}

/// A display name or description: trimmed, 1..=`max` characters, no control
/// characters; empty clears it.
pub fn label_text(value: &str, max: usize) -> Option<Option<String>> {
    let value = value.trim();
    if value.is_empty() {
        return Some(None);
    }
    ((value.chars().count() <= max) && !value.chars().any(char::is_control))
        .then(|| Some(value.to_owned()))
}
pub fn label_name(value: &str) -> Option<Option<String>> {
    label_text(value, NAME_CHARS)
}
pub fn label_description(value: &str) -> Option<Option<String>> {
    label_text(value, DESCRIPTION_CHARS)
}

/// An address or alias as typed: lowercased, one leading `/` dropped, then
/// `[a-z][a-z0-9-]{1,31}`, no `--`, no trailing `-` (case-insensitive rule).
pub fn address_shaped(value: &str) -> Option<String> {
    let value = value.trim().trim_start_matches('/').to_lowercase();
    crate::composer::commands::valid_slug(&value).then_some(value)
}

/// A member's own shortcut as typed: as [`address_shaped`], but one letter
/// is enough (`/e`): verbs come first and the full form `/app:<name>` is
/// never ambiguous, so a short shortcut shadows nothing (2026-10-31).
pub fn shortcut_shaped(value: &str) -> Option<String> {
    let value = value.trim().trim_start_matches('/').to_lowercase();
    let single = value.len() == 1 && value.as_bytes()[0].is_ascii_lowercase();
    (single || crate::composer::commands::valid_slug(&value)).then_some(value)
}

/// A card's name within its app (`ledger`, `by-status`): `[a-z0-9][a-z0-9-]{0,47}`, or empty for the app.
pub fn card_shaped(card: &str) -> bool {
    let bytes = card.as_bytes();
    card.is_empty()
        || (bytes.len() <= 48
            && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
            && bytes
                .iter()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-'))
}

fn text(row: &Row, index: usize) -> Result<String> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}
fn optional(row: &Row, index: usize) -> Result<Option<String>> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}
fn texts(row: &Row, index: usize) -> Result<Vec<String>> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}

/// The domain-separated digest an audit event's outcome carries.
fn outcome(kind: &str, parts: &[&str]) -> String {
    use sha2::{Digest as _, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(format!("forma:label-{kind}:v1\u{0}").as_bytes());
    hasher.update(parts.join("\u{0}").as_bytes());
    format!(
        "sha256:{}",
        hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}

/// Every app of the organization the member can open (`discover`).
pub(super) const DISCOVER: &str = "EXISTS(SELECT 1 FROM genui.app_permissions p WHERE p.organization_id=a.organization_id AND p.principal_id=$2::text::uuid AND p.app_id=a.app_id AND p.permission='discover')";
/// The app is open to someone besides its owner (`discover`).
const SHARED: &str = "EXISTS(SELECT 1 FROM genui.app_permissions p WHERE p.organization_id=a.organization_id AND p.app_id=a.app_id AND p.permission='discover' AND p.principal_id IS DISTINCT FROM a.owner_principal_id)";
/// The member owns the app or holds `admin_policy` in the organization.
pub(super) const OWNER_OR_ADMIN: &str = "(a.owner_principal_id=$2::text::uuid OR EXISTS(SELECT 1 FROM genui.organization_permissions o WHERE o.organization_id=a.organization_id AND o.principal_id=$2::text::uuid AND o.permission='admin_policy'))";

/// The organization's naming lock (transaction-scoped): every label write
/// and every "Save as /name" judges its names under it.
async fn lock_names(transaction: &Transaction<'_>, organization: &str) -> Result<()> {
    let key = format!("genui:labels:{organization}");
    transaction
        .query(
            "SELECT pg_advisory_xact_lock(hashtextextended($1::text,0))::text",
            &[&key],
        )
        .await?;
    Ok(())
}

/// The names storage holds other than `except`'s own labels ([`HeldNames`]),
/// organization-wide: never narrowed to the apps the member can open.
async fn held_names(
    transaction: &Transaction<'_>,
    request: &TrustedRequest,
    except: Option<&str>,
) -> Result<HeldNames> {
    let rows = transaction.query("SELECT l.name,l.address,l.kept FROM genui.app_labels l WHERE l.organization_id=$1::text::uuid AND l.card='' AND ($2::text IS NULL OR l.app_id<>$2::text::uuid)", &[&request.organization,&except]).await?;
    let mut held = HeldNames::default();
    for row in &rows {
        held.titles
            .extend(optional(row, 0)?.map(|name| name.to_lowercase()));
        held.names.extend(optional(row, 1)?);
        held.names.extend(texts(row, 2)?);
    }
    held.names.extend(saved_names(transaction, request).await?);
    Ok(held)
}

/// Every saved work's `/name` in the organization (its context's marker
/// line), whoever may open it.
async fn saved_names(
    transaction: &Transaction<'_>,
    request: &TrustedRequest,
) -> Result<BTreeSet<String>> {
    let pattern = format!("{}%", super::COMMAND_MARKER);
    let rows = transaction.query("SELECT split_part(w.context,E'\\n',1) FROM genui.saved_work w WHERE w.organization_id=$1::text::uuid AND w.context LIKE $2", &[&request.organization,&pattern]).await?;
    let mut names = BTreeSet::new();
    for row in &rows {
        if let Some(slug) = super::command_slug(&text(row, 0)?) {
            names.insert(slug.to_lowercase());
        }
    }
    Ok(names)
}

/// "Save as /name" (`create_work` with a marked context): under the naming
/// lock, whether an organization label's address or alias already holds
/// `slug` (organization-wide, whoever can open that app).
pub(super) async fn label_holds(
    transaction: &Transaction<'_>,
    request: &TrustedRequest,
    slug: &str,
) -> Result<bool> {
    lock_names(transaction, &request.organization).await?;
    let rows = transaction.query("SELECT 1 FROM genui.app_labels l WHERE l.organization_id=$1::text::uuid AND l.card='' AND (l.address=$2 OR $2=ANY(l.kept)) LIMIT 1", &[&request.organization,&slug]).await?;
    Ok(!rows.is_empty())
}

/// The card names (`totals`, `by-status`, `new`, `records`, `ledger`) the
/// canvas gives an app's cards (`library.ts` `cardSlug`) from its published
/// definitions' views: each view id lowercased, runs of other characters a
/// hyphen. Pure; exported for the tests.
pub fn card_bases(contents: &[serde_json::Value]) -> BTreeSet<String> {
    let mut bases: BTreeSet<String> = ["new", "records", "ledger"]
        .iter()
        .map(|base| (*base).to_owned())
        .collect();
    for content in contents {
        for view in content
            .get("views")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(id) = view.get("id").and_then(serde_json::Value::as_str) else {
                continue;
            };
            let mut slug = String::new();
            for ch in id.to_lowercase().chars() {
                if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
                    slug.push(ch);
                } else if !slug.ends_with('-') {
                    slug.push('-');
                }
            }
            let slug = slug.trim_matches('-');
            if !slug.is_empty() {
                bases.insert(slug.to_owned());
            }
        }
    }
    bases
}

/// Whether `card` names one of an app's cards: a base, or a base with the
/// canvas's `-2`, `-3` … suffix for a repeat. Pure; exported for the tests.
pub fn card_known(bases: &BTreeSet<String>, card: &str) -> bool {
    card.is_empty()
        || bases.contains(card)
        || card.rsplit_once('-').is_some_and(|(base, n)| {
            bases.contains(base) && !n.starts_with('0') && n.parse::<u16>().is_ok_and(|n| n >= 2)
        })
}

/// Whether `app` has a card named `card` (its published definitions).
async fn has_card(
    transaction: &Transaction<'_>,
    request: &TrustedRequest,
    app: &str,
    card: &str,
) -> Result<bool> {
    if card.is_empty() {
        return Ok(true);
    }
    let rows = transaction.query("SELECT d.canonical_content FROM genui.publications p JOIN genui.definition_revisions d ON d.organization_id=p.organization_id AND d.app_id=p.app_id AND d.revision_id=p.definition_revision_id WHERE p.organization_id=$1::text::uuid AND p.app_id=$2::text::uuid ORDER BY p.publication_sequence DESC LIMIT 16", &[&request.organization,&app]).await?;
    let mut contents = Vec::with_capacity(rows.len());
    for row in &rows {
        let content: Vec<u8> = row.try_get(0).map_err(|_| Error::Unavailable)?;
        contents.push(crate::admission::decode(&content).map_err(|_| Error::Unavailable)?);
    }
    Ok(card_known(&card_bases(&contents), card))
}

/// Why an organization label `next` (from `current`) may not be written,
/// if it may not (module notes). `shared`: the app is open to someone
/// besides its owner. Pure; exported for the tests.
pub fn label_refusal(
    current: &Label,
    next: &Label,
    rules: &NameRules,
    held: &HeldNames,
    shared: bool,
) -> Option<&'static str> {
    if next.kept.len() > MAX_LABEL_ALIASES {
        return Some("invalid_name");
    }
    let fresh: Vec<&String> = next
        .address
        .iter()
        .chain(&next.kept)
        .filter(|n| current.address.as_ref() != Some(*n) && !current.kept.contains(n))
        .collect();
    if fresh.iter().any(|n| rules.verbs.contains(*n)) {
        return Some("verb_name");
    }
    if !(fresh.is_empty() || rules.library || shared) {
        return Some("not_shared");
    }
    if fresh
        .iter()
        .any(|n| rules.others.contains(*n) || held.names.contains(*n))
    {
        return Some("name_taken");
    }
    if let Some(name) = next.name.as_ref().filter(|_| next.name != current.name) {
        let name = name.to_lowercase();
        if next.card.is_empty()
            && (rules.others.contains(&name)
                || rules.titles.contains(&name)
                || held.names.contains(&name)
                || held.titles.contains(&name))
        {
            return Some("name_taken");
        }
    }
    None
}

/// Why a personal label `next` (from `current`) may not be written, if it
/// may not: a fresh shortcut takes a verb, any organization name (catalog
/// or label, this app's among them), a saved work's `/name` or another of
/// the member's shortcuts (`mine`). Pure; exported for the tests.
pub fn mine_refusal(
    current: &MemberLabel,
    next: &MemberLabel,
    rules: &NameRules,
    held: &HeldNames,
    mine: &BTreeSet<String>,
) -> Option<&'static str> {
    if next.aliases.len() > MAX_LABEL_ALIASES {
        return Some("invalid_name");
    }
    let fresh: Vec<&String> = next
        .aliases
        .iter()
        .filter(|n| !current.aliases.contains(n))
        .collect();
    if fresh.iter().any(|n| rules.verbs.contains(*n)) {
        return Some("verb_name");
    }
    if fresh.iter().any(|n| {
        rules.others.contains(*n)
            || rules.own.contains(n)
            || held.names.contains(*n)
            || mine.contains(*n)
    }) {
        return Some("name_taken");
    }
    None
}

fn dedupe(list: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    list.into_iter()
        .filter(|n| seen.insert(n.clone()))
        .collect()
}

/// The merged organization label after `change` (module notes): renaming
/// the address keeps the previous one (`kept`), an address set back to the
/// catalog's own clears the label's, and the address is never also kept.
/// `change.aliases` is personal only and ignored here. Pure; exported for
/// the tests.
pub fn merge_label(current: &Label, change: &LabelChange, own: &[String]) -> Label {
    let catalog_slug = own.first().cloned();
    let previous = current.address.clone().or_else(|| catalog_slug.clone());
    let mut next = current.clone();
    if let Some(name) = &change.name {
        next.name = name.clone();
    }
    if let Some(description) = &change.description {
        next.description = description.clone();
    }
    if let Some(address) = &change.address {
        let wanted = address.clone().or_else(|| catalog_slug.clone());
        if wanted != previous {
            if let Some(previous) = previous.filter(|p| !own.contains(p)) {
                next.kept.push(previous);
            }
        }
        next.address = wanted.filter(|a| Some(a) != catalog_slug.as_ref());
    }
    let effective = next.address.clone().or(catalog_slug);
    next.kept = dedupe(
        next.kept
            .into_iter()
            .filter(|a| Some(a) != effective.as_ref() && !own.contains(a))
            .collect(),
    );
    next
}

impl Workspace {
    async fn labels_transaction<'a, T, F>(
        &'a self,
        request: &'a TrustedRequest,
        body: F,
    ) -> Result<T>
    where
        F: for<'t> FnOnce(
            &'t mut Transaction<'a>,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<T>> + Send + 't>,
        >,
    {
        let cap = Instant::now() + LABEL_DEADLINE;
        let mut transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = body(&mut transaction).await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// The labels this member sees (module notes): organization labels of
    /// the apps they can open, their own labels on those apps, and the apps
    /// they may label for everyone.
    pub async fn labels(&self, request: &TrustedRequest) -> Result<Labels> {
        self.labels_transaction(request, |transaction| {
            Box::pin(async move {
                confirm_identity(transaction, request).await?;
                let permit = transaction.fence_member().await?;
                let t = permit.transaction();
                let mut labels = Labels::default();
                for row in t.query(&format!("SELECT l.app_id::text,l.card,l.name,l.description,l.address,l.kept FROM genui.app_labels l JOIN genui.apps a USING(organization_id,app_id) WHERE l.organization_id=$1::text::uuid AND {DISCOVER} ORDER BY l.app_id,l.card"), &[&request.organization,&request.principal]).await? {
                    labels.organization.push(Label { app: text(&row, 0)?, card: text(&row, 1)?, name: optional(&row, 2)?, description: optional(&row, 3)?, address: optional(&row, 4)?, kept: texts(&row, 5)? });
                }
                for row in t.query(&format!("SELECT l.app_id::text,l.card,l.name,l.aliases FROM genui.member_labels l JOIN genui.apps a USING(organization_id,app_id) WHERE l.organization_id=$1::text::uuid AND l.principal_id=$2::text::uuid AND {DISCOVER} ORDER BY l.app_id,l.card"), &[&request.organization,&request.principal]).await? {
                    labels.mine.push(MemberLabel { app: text(&row, 0)?, card: text(&row, 1)?, name: optional(&row, 2)?, aliases: texts(&row, 3)? });
                }
                for row in t.query(&format!("SELECT a.app_id::text FROM genui.apps a WHERE a.organization_id=$1::text::uuid AND {DISCOVER} AND {OWNER_OR_ADMIN}"), &[&request.organization,&request.principal]).await? {
                    labels.labelable.insert(text(&row, 0)?);
                }
                Ok(labels)
            })
        })
        .await
    }

    /// Changes the organization label of `app` (or of its `card`): the
    /// app's owner or an admin who can open it, else `Denied`. Audited
    /// `app.labeled`; an unchanged label writes nothing.
    pub async fn label_app(
        &self,
        request: &TrustedRequest,
        app: &str,
        card: &str,
        change: &LabelChange,
        rules: &NameRules,
    ) -> Result<Labeled<Label>> {
        require(valid_id(app) && card_shaped(card))?;
        require(change.aliases.is_none() && (card.is_empty() || change.address.is_none()))?;
        self.labels_transaction(request, |transaction| {
            Box::pin(async move {
                confirm_identity(transaction, request).await?;
                let permit = transaction.fence_member().await?;
                let t = permit.transaction();
                let allowed = t.query(&format!("SELECT 1 FROM genui.apps a WHERE a.organization_id=$1::text::uuid AND a.app_id=$3::text::uuid AND {DISCOVER} AND {OWNER_OR_ADMIN}"), &[&request.organization,&request.principal,&app]).await?;
                require(allowed.len() == 1)?;
                lock_names(t, &request.organization).await?;
                let rows = t.query("SELECT name,description,address,kept FROM genui.app_labels WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND card=$3", &[&request.organization,&app,&card]).await?;
                let current = match rows.first() {
                    Some(row) => Label { app: app.to_owned(), card: card.to_owned(), name: optional(row, 0)?, description: optional(row, 1)?, address: optional(row, 2)?, kept: texts(row, 3)? },
                    None => Label { app: app.to_owned(), card: card.to_owned(), ..Label::default() },
                };
                let next = merge_label(&current, change, &rules.own);
                if next == current {
                    return Ok(Labeled::Unchanged(current));
                }
                if !has_card(t, request, app, card).await? {
                    return Ok(Labeled::Refused("invalid_label"));
                }
                let shared = rules.library || !t.query(&format!("SELECT 1 FROM genui.apps a WHERE a.organization_id=$1::text::uuid AND a.app_id=$2::text::uuid AND {SHARED}"), &[&request.organization,&app]).await?.is_empty();
                let held = held_names(t, request, Some(app)).await?;
                if let Some(code) = label_refusal(&current, &next, rules, &held, shared) {
                    return Ok(Labeled::Refused(code));
                }
                let written = t.query("INSERT INTO genui.app_labels(organization_id,app_id,card,name,description,address,kept,updated_by,updated_at) VALUES ($1::text::uuid,$2::text::uuid,$3,$4,$5,$6,$7,$8::text::uuid,date_trunc('second',now())) ON CONFLICT (organization_id,app_id,card) DO UPDATE SET name=EXCLUDED.name,description=EXCLUDED.description,address=EXCLUDED.address,kept=EXCLUDED.kept,updated_by=EXCLUDED.updated_by,updated_at=EXCLUDED.updated_at RETURNING 1", &[&request.organization,&app,&card,&next.name,&next.description,&next.address,&next.kept,&request.principal]).await?;
                require(written.len() == 1)?;
                let digest = outcome("labeled", &[card, next.name.as_deref().unwrap_or(""), next.description.as_deref().unwrap_or(""), next.address.as_deref().unwrap_or(""), &next.kept.join(",")]);
                append(t, Chain::Workspace, request, Event { kind: "app.labeled", id: None, app: Some(app), work: None, record: None, outcome: Some(&digest) }).await?;
                Ok(Labeled::Saved(next))
            })
        })
        .await
    }

    /// Changes the member's own label of `app` (or of its `card`): their
    /// name for it and their shortcuts, on an app they can open, else
    /// `Denied`. A shortcut never takes a verb, an organization address or
    /// alias (catalog or label), a saved work's `/name` or another of the
    /// member's shortcuts; the card must be one the app has, and a member
    /// keeps at most [`MAX_MEMBER_LABELS`] labels.
    /// Audited `app.aliased`; an unchanged label writes nothing.
    pub async fn label_mine(
        &self,
        request: &TrustedRequest,
        app: &str,
        card: &str,
        name: Option<Option<String>>,
        aliases: Option<Vec<String>>,
        rules: &NameRules,
    ) -> Result<Labeled<MemberLabel>> {
        require(valid_id(app) && card_shaped(card))?;
        self.labels_transaction(request, |transaction| {
            Box::pin(async move {
                confirm_identity(transaction, request).await?;
                let permit = transaction.fence_member().await?;
                let t = permit.transaction();
                let allowed = t.query(&format!("SELECT 1 FROM genui.apps a WHERE a.organization_id=$1::text::uuid AND a.app_id=$3::text::uuid AND {DISCOVER}"), &[&request.organization,&request.principal,&app]).await?;
                require(allowed.len() == 1)?;
                lock_names(t, &request.organization).await?;
                let rows = t.query("SELECT name,aliases FROM genui.member_labels WHERE organization_id=$1::text::uuid AND principal_id=$2::text::uuid AND app_id=$3::text::uuid AND card=$4", &[&request.organization,&request.principal,&app,&card]).await?;
                let current = match rows.first() {
                    Some(row) => MemberLabel { app: app.to_owned(), card: card.to_owned(), name: optional(row, 0)?, aliases: texts(row, 1)? },
                    None => MemberLabel { app: app.to_owned(), card: card.to_owned(), ..MemberLabel::default() },
                };
                let mut next = current.clone();
                if let Some(name) = name {
                    next.name = name;
                }
                if let Some(aliases) = aliases {
                    next.aliases = dedupe(aliases);
                }
                if next == current {
                    return Ok(Labeled::Unchanged(current));
                }
                if !has_card(t, request, app, card).await? {
                    return Ok(Labeled::Refused("invalid_label"));
                }
                if rows.is_empty() {
                    let count = t.query("SELECT count(*) FROM genui.member_labels WHERE organization_id=$1::text::uuid AND principal_id=$2::text::uuid", &[&request.organization,&request.principal]).await?;
                    let count: i64 = count.first().ok_or(Error::Unavailable)?.try_get(0).map_err(|_| Error::Unavailable)?;
                    if count >= MAX_MEMBER_LABELS {
                        return Ok(Labeled::Refused("too_many_labels"));
                    }
                }
                let held = held_names(t, request, None).await?;
                let others = t.query("SELECT aliases FROM genui.member_labels WHERE organization_id=$1::text::uuid AND principal_id=$2::text::uuid AND NOT (app_id=$3::text::uuid AND card=$4)", &[&request.organization,&request.principal,&app,&card]).await?;
                let mut mine = BTreeSet::new();
                for row in &others {
                    mine.extend(texts(row, 0)?);
                }
                if let Some(code) = mine_refusal(&current, &next, rules, &held, &mine) {
                    return Ok(Labeled::Refused(code));
                }
                let written = t.query("INSERT INTO genui.member_labels(organization_id,principal_id,app_id,card,name,aliases,updated_at) VALUES ($1::text::uuid,$2::text::uuid,$3::text::uuid,$4,$5,$6,date_trunc('second',now())) ON CONFLICT (organization_id,principal_id,app_id,card) DO UPDATE SET name=EXCLUDED.name,aliases=EXCLUDED.aliases,updated_at=EXCLUDED.updated_at RETURNING 1", &[&request.organization,&request.principal,&app,&card,&next.name,&next.aliases]).await?;
                require(written.len() == 1)?;
                let digest = outcome("aliased", &[card, next.name.as_deref().unwrap_or(""), &next.aliases.join(",")]);
                append(t, Chain::Workspace, request, Event { kind: "app.aliased", id: None, app: Some(app), work: None, record: None, outcome: Some(&digest) }).await?;
                Ok(Labeled::Saved(next))
            })
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn own() -> Vec<String> {
        vec!["expenses".into(), "expense".into(), "claim".into()]
    }

    #[test]
    fn renaming_the_address_keeps_the_old_one() {
        let start = Label::default();
        let renamed = merge_label(
            &start,
            &LabelChange {
                name: Some(Some("Team spend".into())),
                address: Some(Some("team-spend".into())),
                ..LabelChange::default()
            },
            &own(),
        );
        assert_eq!(renamed.name.as_deref(), Some("Team spend"));
        assert_eq!(renamed.address.as_deref(), Some("team-spend"));
        // The catalog's /expenses and /expense stay without being copied.
        assert!(renamed.kept.is_empty());
        // A second rename keeps /team-spend as an alias.
        let again = merge_label(
            &renamed,
            &LabelChange {
                address: Some(Some("spend-team".into())),
                ..LabelChange::default()
            },
            &own(),
        );
        assert_eq!(again.address.as_deref(), Some("spend-team"));
        assert_eq!(again.kept, ["team-spend"]);
        // Back to the catalog's own address: the label's is cleared, the
        // address is never also an alias.
        let back = merge_label(
            &again,
            &LabelChange {
                address: Some(Some("team-spend".into())),
                ..LabelChange::default()
            },
            &own(),
        );
        assert_eq!(back.address.as_deref(), Some("team-spend"));
        assert_eq!(back.kept, ["spend-team"]);
        let catalog = merge_label(
            &back,
            &LabelChange {
                address: Some(None),
                ..LabelChange::default()
            },
            &own(),
        );
        assert_eq!(catalog.address, None);
        assert_eq!(catalog.kept, ["spend-team", "team-spend"]);
        // Unchanged input merges to the same label (the write is skipped).
        assert_eq!(
            merge_label(&catalog, &LabelChange::default(), &own()),
            catalog
        );
    }

    fn rules() -> NameRules {
        NameRules {
            own: own(),
            others: [
                "vendor-spend-breakdown",
                "spend",
                "vendors",
                "time-off",
                "pto",
            ]
            .map(str::to_owned)
            .into(),
            titles: ["vendor spend breakdown", "time off"]
                .map(str::to_owned)
                .into(),
            verbs: ["card", "agents", "storage"].map(str::to_owned).into(),
            library: true,
        }
    }

    fn app(name: Option<&str>, address: Option<&str>, kept: &[&str]) -> Label {
        Label {
            name: name.map(str::to_owned),
            address: address.map(str::to_owned),
            kept: kept.iter().map(|a| (*a).to_owned()).collect(),
            ..Label::default()
        }
    }

    #[test]
    fn a_label_never_takes_a_name_another_app_answers_to() {
        let start = Label::default();
        let held = HeldNames {
            names: ["over-200", "ops"].map(str::to_owned).into(),
            titles: ["operations".to_owned()].into(),
        };
        let judge = |next: &Label| label_refusal(&start, next, &rules(), &held, false);
        assert_eq!(
            judge(&app(Some("Team spend"), Some("team-spend"), &[])),
            None
        );
        // Another app's hidden alias, its address, another label's current
        // or kept address, a saved work's /name: all taken (finding 1, 5).
        for (address, kept) in [
            (Some("spend"), &[][..]),
            (None, &["vendors"][..]),
            (Some("vendor-spend-breakdown"), &[][..]),
            (None, &["ops"][..]),
            (Some("over-200"), &[][..]),
        ] {
            assert_eq!(
                judge(&app(None, address, kept)),
                Some("name_taken"),
                "{address:?} {kept:?}"
            );
        }
        // A verb alias of the canvas (finding 4).
        assert_eq!(judge(&app(None, Some("agents"), &[])), Some("verb_name"));
        assert_eq!(judge(&app(None, None, &["storage"])), Some("verb_name"));
        // A display name never another app's address or title (finding 2).
        for name in ["Time off", "TIME-OFF", "Operations", "Spend"] {
            assert_eq!(
                judge(&app(Some(name), None, &[])),
                Some("name_taken"),
                "{name}"
            );
        }
        // A card's name is only a label of that card.
        let card = Label {
            card: "totals".into(),
            name: Some("Time off".into()),
            ..Label::default()
        };
        assert_eq!(judge(&card), None);
        // Kept names are never judged again; an unchanged name neither.
        let current = app(Some("Time off"), Some("spend"), &[]);
        assert_eq!(
            label_refusal(&current, &current.clone(), &rules(), &held, false),
            None
        );
        // A private app gets no address (finding 3); a shared one does.
        let private = NameRules {
            library: false,
            ..rules()
        };
        let next = app(None, Some("my-tool"), &[]);
        assert_eq!(
            label_refusal(&start, &next, &private, &held, false),
            Some("not_shared")
        );
        assert_eq!(label_refusal(&start, &next, &private, &held, true), None);
        assert_eq!(
            label_refusal(
                &start,
                &app(Some("My tool"), None, &[]),
                &private,
                &held,
                false
            ),
            None
        );
    }

    #[test]
    fn a_shortcut_takes_no_organization_name() {
        let held = HeldNames {
            names: ["team-spend", "over-200"].map(str::to_owned).into(),
            titles: BTreeSet::new(),
        };
        let start = MemberLabel::default();
        let mine: BTreeSet<String> = ["q3".to_owned()].into();
        let judge = |aliases: &[&str]| {
            let next = MemberLabel {
                aliases: aliases.iter().map(|a| (*a).to_owned()).collect(),
                ..MemberLabel::default()
            };
            mine_refusal(&start, &next, &rules(), &held, &mine)
        };
        assert_eq!(judge(&["exp"]), None);
        for taken in ["spend", "expense", "team-spend", "over-200", "q3"] {
            assert_eq!(judge(&["exp", taken]), Some("name_taken"), "{taken}");
        }
        assert_eq!(judge(&["card"]), Some("verb_name"));
    }

    #[test]
    fn cards_are_the_ones_the_app_has() {
        let content = serde_json::json!({"views": [{"id": "totals"}, {"id": "by_status"}, {"id": "Recent!"}]});
        let bases = card_bases(&[content]);
        for card in [
            "",
            "totals",
            "by-status",
            "recent",
            "new",
            "records",
            "ledger",
            "records-2",
            "new-12",
        ] {
            assert!(card_known(&bases, card), "{card}");
        }
        for card in [
            "total",
            "nope",
            "totals-1",
            "totals-02",
            "by-status-x",
            "app",
        ] {
            assert!(!card_known(&bases, card), "{card}");
        }
    }

    #[test]
    fn names_are_shaped() {
        assert_eq!(address_shaped("/Spend").as_deref(), Some("spend"));
        assert_eq!(address_shaped("Q3").as_deref(), Some("q3"));
        assert_eq!(address_shaped("a"), None);
        assert_eq!(shortcut_shaped("/E").as_deref(), Some("e"));
        assert_eq!(shortcut_shaped("q3").as_deref(), Some("q3"));
        assert_eq!(shortcut_shaped("3"), None);
        assert_eq!(shortcut_shaped("-"), None);
        assert_eq!(shortcut_shaped(""), None);
        assert_eq!(address_shaped("team spend"), None);
        assert_eq!(label_name("  Team spend "), Some(Some("Team spend".into())));
        assert_eq!(label_name(""), Some(None));
        assert_eq!(label_name(&"x".repeat(61)), None);
        assert_eq!(label_name("a\u{7}b"), None);
        assert!(card_shaped("") && card_shaped("ledger") && card_shaped("by-status"));
        assert!(!card_shaped("Ledger") && !card_shaped("-x") && !card_shaped(&"a".repeat(49)));
        assert_ne!(
            outcome("labeled", &["a", "b"]),
            outcome("aliased", &["a", "b"])
        );
    }
}
