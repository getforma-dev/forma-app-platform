//! APP-121 (ADR-011 pitch, `pivot/naming`): names people give apps and
//! cards (migration 0014).
//!
//! Two layers. **Organization labels** (`app_labels`): an app's display name,
//! address, kept aliases and description, and each card's display name and
//! description, written only by the app's owner or a holder of
//! `admin_policy` who can open the app (`discover`), audited `app.labeled`.
//! **Personal labels** (`member_labels`): one member's own shortcuts (`/exp`
//! → an app, `/q3` → a card) and own card names, written by that member on
//! an app they can open, audited `app.aliased`, read back only for them.
//!
//! Labels never touch a definition, revision, publication, record or
//! receipt; without a row every surface falls back to the catalog's names.
//! Every operation is one READ COMMITTED workspace transaction under the
//! member fence (`fence_member`) and the request's RLS scope, so a foreign
//! organization's apps are unreachable and every refusal is `Denied` (the
//! route's uniform 404). Name rules (organization addresses unique and
//! never a verb; a personal alias never takes an organization name) are
//! judged under a per-organization advisory lock, so two concurrent label
//! writes cannot both take one name; the address is also unique in storage.
//! A write whose result equals the stored row writes nothing and appends no
//! event (idempotent).
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
/// At most this many kept aliases (organization) or shortcuts (personal) per target.
pub const MAX_LABEL_ALIASES: usize = 8;
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
    pub aliases: Vec<String>,
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

/// A change: `None` keeps the field, `Some(None)` clears it, `Some(Some(v))` sets it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LabelChange {
    pub name: Option<Option<String>>,
    pub description: Option<Option<String>>,
    pub address: Option<Option<String>>,
    pub aliases: Option<Vec<String>>,
}

/// The names a write is judged against that storage does not hold: the
/// catalog's address and aliases of the target app (`own`, always kept),
/// every other app's catalog address (`others`) and hidden catalog aliases
/// (`other_aliases`), and the reserved verbs. An organization address or
/// alias may take another app's hidden catalog alias (it then answers for
/// the labeled app: an address beats an alias), never its address; a
/// personal shortcut takes neither.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NameRules {
    pub own: Vec<String>,
    pub others: BTreeSet<String>,
    pub other_aliases: BTreeSet<String>,
    pub verbs: BTreeSet<String>,
}

/// The outcome of a label write the member was allowed to make.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Labeled<T> {
    /// Written and audited.
    Saved(T),
    /// The stored label already said this; nothing was written.
    Unchanged(T),
    /// A rule refused it: `invalid_name`, `verb_name` or `name_taken`.
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
const DISCOVER: &str = "EXISTS(SELECT 1 FROM genui.app_permissions p WHERE p.organization_id=a.organization_id AND p.principal_id=$2::text::uuid AND p.app_id=a.app_id AND p.permission='discover')";
/// The member owns the app or holds `admin_policy` in the organization.
const OWNER_OR_ADMIN: &str = "(a.owner_principal_id=$2::text::uuid OR EXISTS(SELECT 1 FROM genui.organization_permissions o WHERE o.organization_id=a.organization_id AND o.principal_id=$2::text::uuid AND o.permission='admin_policy'))";

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

/// The organization's label names other than `app`'s own (addresses and aliases).
async fn organization_names(
    transaction: &Transaction<'_>,
    request: &TrustedRequest,
    except: Option<&str>,
) -> Result<BTreeSet<String>> {
    let rows = transaction.query("SELECT l.address,l.aliases FROM genui.app_labels l WHERE l.organization_id=$1::text::uuid AND l.card='' AND ($2::text IS NULL OR l.app_id<>$2::text::uuid)", &[&request.organization,&except]).await?;
    let mut names = BTreeSet::new();
    for row in &rows {
        names.extend(optional(row, 0)?);
        names.extend(texts(row, 1)?);
    }
    Ok(names)
}

fn dedupe(list: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    list.into_iter()
        .filter(|n| seen.insert(n.clone()))
        .collect()
}

/// The merged organization label after `change` (module notes): renaming
/// the address keeps the previous one as an alias, an address set back to
/// the catalog's own clears the label's, and the address is never also an
/// alias. Pure; exported for the tests.
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
    if let Some(aliases) = &change.aliases {
        next.aliases = aliases.clone();
    }
    if let Some(address) = &change.address {
        let wanted = address.clone().or_else(|| catalog_slug.clone());
        if wanted != previous {
            if let Some(previous) = previous.filter(|p| !own.contains(p)) {
                next.aliases.push(previous);
            }
        }
        next.address = wanted.filter(|a| Some(a) != catalog_slug.as_ref());
    }
    let effective = next.address.clone().or(catalog_slug);
    next.aliases = dedupe(
        next.aliases
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
                for row in t.query(&format!("SELECT l.app_id::text,l.card,l.name,l.description,l.address,l.aliases FROM genui.app_labels l JOIN genui.apps a USING(organization_id,app_id) WHERE l.organization_id=$1::text::uuid AND {DISCOVER} ORDER BY l.app_id,l.card"), &[&request.organization,&request.principal]).await? {
                    labels.organization.push(Label { app: text(&row, 0)?, card: text(&row, 1)?, name: optional(&row, 2)?, description: optional(&row, 3)?, address: optional(&row, 4)?, aliases: texts(&row, 5)? });
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
        require(card.is_empty() || (change.address.is_none() && change.aliases.is_none()))?;
        self.labels_transaction(request, |transaction| {
            Box::pin(async move {
                confirm_identity(transaction, request).await?;
                let permit = transaction.fence_member().await?;
                let t = permit.transaction();
                let allowed = t.query(&format!("SELECT 1 FROM genui.apps a WHERE a.organization_id=$1::text::uuid AND a.app_id=$3::text::uuid AND {DISCOVER} AND {OWNER_OR_ADMIN}"), &[&request.organization,&request.principal,&app]).await?;
                require(allowed.len() == 1)?;
                lock_names(t, &request.organization).await?;
                let rows = t.query("SELECT name,description,address,aliases FROM genui.app_labels WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND card=$3", &[&request.organization,&app,&card]).await?;
                let current = match rows.first() {
                    Some(row) => Label { app: app.to_owned(), card: card.to_owned(), name: optional(row, 0)?, description: optional(row, 1)?, address: optional(row, 2)?, aliases: texts(row, 3)? },
                    None => Label { app: app.to_owned(), card: card.to_owned(), ..Label::default() },
                };
                let next = merge_label(&current, change, &rules.own);
                if next == current {
                    return Ok(Labeled::Unchanged(current));
                }
                if next.aliases.len() > MAX_LABEL_ALIASES {
                    return Ok(Labeled::Refused("invalid_name"));
                }
                let fresh: Vec<&String> = next.address.iter().chain(&next.aliases)
                    .filter(|n| current.address.as_ref() != Some(*n) && !current.aliases.contains(n))
                    .collect();
                if fresh.iter().any(|n| rules.verbs.contains(*n)) {
                    return Ok(Labeled::Refused("verb_name"));
                }
                let taken = organization_names(t, request, Some(app)).await?;
                if fresh.iter().any(|n| rules.others.contains(*n) || taken.contains(*n)) {
                    return Ok(Labeled::Refused("name_taken"));
                }
                let written = t.query("INSERT INTO genui.app_labels(organization_id,app_id,card,name,description,address,aliases,updated_by,updated_at) VALUES ($1::text::uuid,$2::text::uuid,$3,$4,$5,$6,$7,$8::text::uuid,date_trunc('second',now())) ON CONFLICT (organization_id,app_id,card) DO UPDATE SET name=EXCLUDED.name,description=EXCLUDED.description,address=EXCLUDED.address,aliases=EXCLUDED.aliases,updated_by=EXCLUDED.updated_by,updated_at=EXCLUDED.updated_at RETURNING 1", &[&request.organization,&app,&card,&next.name,&next.description,&next.address,&next.aliases,&request.principal]).await?;
                require(written.len() == 1)?;
                let digest = outcome("labeled", &[card, next.name.as_deref().unwrap_or(""), next.description.as_deref().unwrap_or(""), next.address.as_deref().unwrap_or(""), &next.aliases.join(",")]);
                append(t, Chain::Workspace, request, Event { kind: "app.labeled", id: None, app: Some(app), work: None, record: None, outcome: Some(&digest) }).await?;
                Ok(Labeled::Saved(next))
            })
        })
        .await
    }

    /// Changes the member's own label of `app` (or of its `card`): their
    /// name for it and their shortcuts, on an app they can open, else
    /// `Denied`. A shortcut never takes a verb, an organization address or
    /// alias (catalog or label), or another of the member's shortcuts.
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
                if next.aliases.len() > MAX_LABEL_ALIASES {
                    return Ok(Labeled::Refused("invalid_name"));
                }
                let fresh: Vec<&String> = next.aliases.iter().filter(|n| !current.aliases.contains(n)).collect();
                if fresh.iter().any(|n| rules.verbs.contains(*n)) {
                    return Ok(Labeled::Refused("verb_name"));
                }
                let organization = organization_names(t, request, None).await?;
                let mine = t.query("SELECT aliases FROM genui.member_labels WHERE organization_id=$1::text::uuid AND principal_id=$2::text::uuid AND NOT (app_id=$3::text::uuid AND card=$4)", &[&request.organization,&request.principal,&app,&card]).await?;
                let mut own = BTreeSet::new();
                for row in &mine {
                    own.extend(texts(row, 0)?);
                }
                if fresh.iter().any(|n| rules.others.contains(*n) || rules.other_aliases.contains(*n) || rules.own.contains(n) || organization.contains(*n) || own.contains(*n)) {
                    return Ok(Labeled::Refused("name_taken"));
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
                address: Some(Some("spend".into())),
                ..LabelChange::default()
            },
            &own(),
        );
        assert_eq!(renamed.name.as_deref(), Some("Team spend"));
        assert_eq!(renamed.address.as_deref(), Some("spend"));
        // The catalog's /expenses and /expense stay without being copied.
        assert!(renamed.aliases.is_empty());
        // A second rename keeps /spend as an alias.
        let again = merge_label(
            &renamed,
            &LabelChange {
                address: Some(Some("team-spend".into())),
                ..LabelChange::default()
            },
            &own(),
        );
        assert_eq!(again.address.as_deref(), Some("team-spend"));
        assert_eq!(again.aliases, ["spend"]);
        // Back to the catalog's own address: the label's is cleared, the
        // address is never also an alias.
        let back = merge_label(
            &again,
            &LabelChange {
                address: Some(Some("spend".into())),
                ..LabelChange::default()
            },
            &own(),
        );
        assert_eq!(back.address.as_deref(), Some("spend"));
        assert_eq!(back.aliases, ["team-spend"]);
        let catalog = merge_label(
            &back,
            &LabelChange {
                address: Some(None),
                ..LabelChange::default()
            },
            &own(),
        );
        assert_eq!(catalog.address, None);
        assert_eq!(catalog.aliases, ["team-spend", "spend"]);
        // Unchanged input merges to the same label (the write is skipped).
        assert_eq!(
            merge_label(&catalog, &LabelChange::default(), &own()),
            catalog
        );
    }

    #[test]
    fn names_are_shaped() {
        assert_eq!(address_shaped("/Spend").as_deref(), Some("spend"));
        assert_eq!(address_shaped("Q3").as_deref(), Some("q3"));
        assert_eq!(address_shaped("a"), None);
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
