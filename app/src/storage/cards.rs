//! APP-123 (ADR-011 pitch, `pivot/app123`): cards made by you (migration
//! 0015).
//!
//! A **made card** (`kind` `card`) is a view a member made with Shape ("Add
//! as new card", or "Make it a card" on an Ask answer): it belongs to its
//! app and work, names the card it came from and stores only its closed
//! view spec (the route judged it against the work's published schema; this
//! module stores the canonical bytes it is given). A **pinned answer**
//! (`kind` `answer`) is what Ask answered, kept on the canvas: question,
//! text, references and their storage receipts (the route checked each
//! receipt against the record revisions, [`Data::receipts_known`]); it never
//! runs again.
//!
//! A card is its maker's own until the app's owner or an admin who made it
//! adds it for everyone ([`Workspace::share_card`]). A listing
//! ([`Workspace::member_cards`]) shows a card only while the reader could
//! store it: `discover` on its app and `record_read` on its work (the
//! `record_read` clause of the permissions check: a work grant or the
//! creator, `run`, live `source_read` on every dependency), and a pinned
//! answer only while the reader holds the same on every (app, work) its
//! receipts cite (`read_apps`/`read_works`); a card added for everyone
//! lists only while its maker is still a member. Removing is a soft delete
//! by its maker ([`Workspace::remove_card`]); the app's owner or an
//! `admin_policy` holder may also take a card added for everyone back to its
//! maker or remove it, also when its maker left (the steward path: the
//! genui_admit definer `genui_private.steward_card`, since the RESTRICTIVE
//! `member_cards_principal` policy keeps the workspace's own writes to the
//! member's rows). Every operation is one READ COMMITTED workspace
//! transaction under the member fence and the request's RLS scope, so a
//! foreign organization's cards and apps are unreachable and every refusal
//! is `Denied` (the route's uniform 404).
//! Writes are audited on the workspace chain in the same transaction
//! (`card.made`, `answer.pinned`, `card.shared`, `card.removed`) and are
//! idempotent: the client names the card's id, a repeat of the same card
//! writes nothing, and an unchanged share or a second remove writes nothing.
//! A member keeps at most [`MAX_MEMBER_CARDS`] cards and answers, counted
//! under a per-member advisory lock.
//!
//! Accepted (review 2026-10-31, finding 4): storing under an id another
//! member's card already holds is the uniform 404 where a fresh id stores,
//! so the route is an existence oracle for card ids; using it needs that
//! card's client-generated UUIDv4, and it discloses nothing else.
use super::{
    authorization::Transaction,
    boxes::serialize_member,
    definitions::{append, confirm_identity, Chain, Event},
    labels::{DISCOVER, OWNER_OR_ADMIN},
    pool::{Data, Workspace},
    queries::listable,
    require, valid_id, Error, Result, TrustedRequest, QUERY_HANDLER_DEADLINE,
};
use serde_json::Value;
use std::time::Duration;
use tokio::time::Instant;
use tokio_postgres::Row;

/// ADR-007 D7.14 handler deadline for each transaction here.
pub const CARD_DEADLINE: Duration = Duration::from_secs(5);
/// At most this many live (not removed) cards and pinned answers per member.
pub const MAX_MEMBER_CARDS: i64 = 100;
/// At most this many cards one listing returns (the member's own and those
/// added for everyone).
const LIST_LIMIT: i64 = 300;
/// The stored spec's byte bound (the migration's CHECK).
pub const MAX_SPEC_BYTES: usize = 262_144;

/// A card as one member reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct MemberCard {
    pub card: String,
    /// `card` (a made view) or `answer` (a pinned answer).
    pub kind: String,
    pub app: Option<String>,
    pub work: Option<String>,
    /// The card it was made from (`ledger`, `records`), empty for an answer.
    pub source: String,
    pub address: String,
    pub title: String,
    pub spec: Value,
    pub everyone: bool,
    /// The reader made it.
    pub mine: bool,
    /// The reader made it and may add it for everyone (the app's owner or an admin).
    pub can_share: bool,
    /// The reader did not make it, it is added for everyone, and they may
    /// take it back to its maker or remove it (the app's owner or an admin).
    pub can_manage: bool,
    /// Whole-second UTC.
    pub created_at: String,
}

/// A card to store: the route judged every field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewCard {
    pub card: String,
    pub kind: &'static str,
    pub app: Option<String>,
    pub work: Option<String>,
    pub source: String,
    pub address: String,
    pub title: String,
    /// The spec's forma-json-v1 bytes.
    pub spec: Vec<u8>,
    /// A pinned answer's (app, work) pairs, those its receipts cite (empty
    /// for a made card); the listing re-checks `record_read` on each.
    pub reads: Vec<(String, String)>,
}

/// The outcome of a card write the member was allowed to make.
#[derive(Clone, Debug, PartialEq)]
pub enum Carded<T> {
    /// Written and audited.
    Saved(T),
    /// Already so; nothing was written.
    Unchanged(T),
    /// A rule refused it: `card_exists` (this id holds another card),
    /// `too_many_cards` or `not_shareable` (a pinned answer is never added
    /// for everyone).
    Refused(&'static str),
}

/// One receipt a pinned answer cites, as storage would have issued it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceiptKey {
    pub app: String,
    pub work: String,
    pub collection: String,
    pub record: String,
    pub version: i64,
    pub commit_sequence: i64,
    pub publication: String,
    pub definition_digest: String,
}

fn text(row: &Row, index: usize) -> Result<String> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}
fn optional(row: &Row, index: usize) -> Result<Option<String>> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}
fn flag(row: &Row, index: usize) -> Result<bool> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}

/// The domain-separated digest an audit event's outcome carries.
fn outcome(kind: &str, parts: &[&str]) -> String {
    use sha2::{Digest as _, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(format!("forma:member-card-{kind}:v1\u{0}").as_bytes());
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

/// The spec bytes' own digest (what an audit outcome names, never the spec).
fn spec_digest(spec: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    Sha256::digest(spec)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The listing's columns over `genui.member_cards c` (`$2` the reader).
const CARD_COLUMNS: &str = "c.card_id::text,c.kind,c.app_id::text,c.work_id::text,c.source,c.address,c.title,c.spec,c.everyone,c.principal_id=$2::text::uuid,to_char(c.created_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')";

/// A card row as `reader` sees it; `steward`: the reader owns its app or
/// holds `admin_policy`.
fn card_row(row: &Row, steward: bool) -> Result<MemberCard> {
    let spec: Vec<u8> = row.try_get(7).map_err(|_| Error::Unavailable)?;
    let mine = flag(row, 9)?;
    let everyone = flag(row, 8)?;
    let kind = text(row, 1)?;
    Ok(MemberCard {
        card: text(row, 0)?,
        app: optional(row, 2)?,
        work: optional(row, 3)?,
        source: text(row, 4)?,
        address: text(row, 5)?,
        title: text(row, 6)?,
        spec: crate::admission::decode(&spec).map_err(|_| Error::Unavailable)?,
        everyone,
        can_share: mine && steward && kind == "card",
        can_manage: !mine && everyone && steward && kind == "card",
        mine,
        kind,
        created_at: text(row, 10)?,
    })
}

impl Workspace {
    async fn cards_transaction<'a, T, F>(
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
        let cap = Instant::now() + CARD_DEADLINE;
        let mut transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = body(&mut transaction).await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// The cards this member sees (module notes): their own, and those added
    /// for everyone by a maker who is still a member, while the member holds
    /// `discover` on the card's app and `record_read` on its work, and on
    /// every (app, work) a pinned answer's receipts cite; removed ones never.
    pub async fn member_cards(&self, request: &TrustedRequest) -> Result<Vec<MemberCard>> {
        self.cards_transaction(request, |transaction| {
            Box::pin(async move {
                confirm_identity(transaction, request).await?;
                let permit = transaction.fence_member().await?;
                let t = permit.transaction();
                let readable = listable("$2::text::uuid");
                let rows = t.query(&format!("SELECT {CARD_COLUMNS},(c.app_id IS NOT NULL AND EXISTS(SELECT 1 FROM genui.apps a WHERE a.organization_id=c.organization_id AND a.app_id=c.app_id AND {OWNER_OR_ADMIN})) FROM genui.member_cards c WHERE c.organization_id=$1::text::uuid AND c.removed_at IS NULL AND (c.principal_id=$2::text::uuid OR (c.everyone AND EXISTS(SELECT 1 FROM genui.memberships m WHERE m.organization_id=c.organization_id AND m.principal_id=c.principal_id AND m.status='active'))) AND (c.app_id IS NULL OR EXISTS(SELECT 1 FROM genui.saved_work w WHERE w.organization_id=c.organization_id AND w.app_id=c.app_id AND w.work_id=c.work_id AND {readable})) AND NOT EXISTS(SELECT 1 FROM unnest(c.read_apps,c.read_works) AS k(app_id,work_id) WHERE NOT EXISTS(SELECT 1 FROM genui.saved_work w WHERE w.organization_id=c.organization_id AND w.app_id=k.app_id AND w.work_id=k.work_id AND {readable})) ORDER BY c.created_at,c.card_id LIMIT $3"), &[&request.organization,&request.principal,&LIST_LIMIT]).await?;
                rows.iter().map(|row| card_row(row, flag(row, 11)?)).collect()
            })
        })
        .await
    }

    /// Stores a made card or a pinned answer (module notes): on an app the
    /// member can open (an answer over several apps has none), else
    /// `Denied`. Audited `card.made` or `answer.pinned`; the same card again
    /// writes nothing. The member's count is judged under their cards lock.
    pub async fn make_card(
        &self,
        request: &TrustedRequest,
        card: &NewCard,
    ) -> Result<Carded<MemberCard>> {
        require(
            valid_id(&card.card)
                && matches!(card.kind, "card" | "answer")
                && card.app.is_some() == card.work.is_some()
                && card.app.as_deref().is_none_or(valid_id)
                && card.work.as_deref().is_none_or(valid_id)
                && (card.kind == "answer" || card.app.is_some())
                && (card.kind == "answer" || card.reads.is_empty())
                && card.reads.len() <= super::READ_ONLY_WORKS
                && card
                    .reads
                    .iter()
                    .all(|(app, work)| valid_id(app) && valid_id(work))
                && card
                    .reads
                    .iter()
                    .enumerate()
                    .all(|(i, pair)| !card.reads[..i].contains(pair))
                && (2..=MAX_SPEC_BYTES).contains(&card.spec.len()),
        )?;
        let read_apps: Vec<&str> = card.reads.iter().map(|(app, _)| app.as_str()).collect();
        let read_works: Vec<&str> = card.reads.iter().map(|(_, work)| work.as_str()).collect();
        self.cards_transaction(request, |transaction| {
            Box::pin(async move {
                confirm_identity(transaction, request).await?;
                let permit = transaction.fence_member().await?;
                let t = permit.transaction();
                // Review finding 3: one member's card writes are serialized
                // before the count, so two cannot both pass 99.
                serialize_member(t, request, "cards").await?;
                let mut steward = false;
                if let Some(app) = &card.app {
                    let allowed = t.query(&format!("SELECT {OWNER_OR_ADMIN} FROM genui.apps a WHERE a.organization_id=$1::text::uuid AND a.app_id=$3::text::uuid AND {DISCOVER}"), &[&request.organization,&request.principal,app]).await?;
                    steward = flag(allowed.first().ok_or(Error::Denied)?, 0)?;
                }
                let existing = t.query(&format!("SELECT {CARD_COLUMNS},c.removed_at IS NULL,c.read_apps=$4::text[]::uuid[] AND c.read_works=$5::text[]::uuid[] FROM genui.member_cards c WHERE c.organization_id=$1::text::uuid AND c.card_id=$3::text::uuid AND c.principal_id=$2::text::uuid"), &[&request.organization,&request.principal,&card.card,&read_apps,&read_works]).await?;
                if let Some(row) = existing.first() {
                    let stored: Vec<u8> = row.try_get(7).map_err(|_| Error::Unavailable)?;
                    let same = text(row, 1)? == card.kind
                        && optional(row, 2)? == card.app
                        && optional(row, 3)? == card.work
                        && text(row, 4)? == card.source
                        && text(row, 5)? == card.address
                        && text(row, 6)? == card.title
                        && stored == card.spec
                        && flag(row, 11)?
                        && flag(row, 12)?;
                    return Ok(if same {
                        Carded::Unchanged(card_row(row, steward)?)
                    } else {
                        Carded::Refused("card_exists")
                    });
                }
                let count = t.query("SELECT count(*) FROM genui.member_cards WHERE organization_id=$1::text::uuid AND principal_id=$2::text::uuid AND removed_at IS NULL", &[&request.organization,&request.principal]).await?;
                let count: i64 = count.first().ok_or(Error::Unavailable)?.try_get(0).map_err(|_| Error::Unavailable)?;
                if count >= MAX_MEMBER_CARDS {
                    return Ok(Carded::Refused("too_many_cards"));
                }
                // Another member's card under this id is invisible here: the
                // insert does nothing, and that is the uniform refusal. A card
                // starts private (`everyone` is not in the INSERT grant).
                let written = t.query(&format!("INSERT INTO genui.member_cards AS c(organization_id,card_id,principal_id,kind,app_id,work_id,source,address,title,spec,read_apps,read_works,created_at,updated_at) VALUES ($1::text::uuid,$3::text::uuid,$2::text::uuid,$4,$5::text::uuid,$6::text::uuid,$7,$8,$9,$10,$11::text[]::uuid[],$12::text[]::uuid[],date_trunc('second',now()),date_trunc('second',now())) ON CONFLICT (organization_id,card_id) DO NOTHING RETURNING {CARD_COLUMNS}"), &[&request.organization,&request.principal,&card.card,&card.kind,&card.app,&card.work,&card.source,&card.address,&card.title,&card.spec,&read_apps,&read_works]).await?;
                let row = written.first().ok_or(Error::Denied)?;
                let made = card_row(row, steward)?;
                let digest = outcome(card.kind, &[&card.card, &card.source, &card.address, &card.title, &spec_digest(&card.spec)]);
                let kind = if card.kind == "card" { "card.made" } else { "answer.pinned" };
                append(t, Chain::Workspace, request, Event { kind, id: None, app: card.app.as_deref(), work: card.work.as_deref(), record: None, outcome: Some(&digest) }).await?;
                Ok(Carded::Saved(made))
            })
        })
        .await
    }

    /// Adds the member's own made card for everyone who can read its app's
    /// work (the app's owner or an admin who can open it), or keeps it for
    /// them only (`everyone` false; its maker, whoever owns the app now).
    /// The steward path: the app's owner or an admin who can open it takes
    /// another member's card added for everyone back to its maker
    /// (`everyone` false only), through the definer
    /// `genui_private.steward_card`. Anything else is `Denied`. Audited
    /// `card.shared`; an unchanged share writes nothing.
    pub async fn share_card(
        &self,
        request: &TrustedRequest,
        card: &str,
        everyone: bool,
    ) -> Result<Carded<MemberCard>> {
        require(valid_id(card))?;
        self.cards_transaction(request, |transaction| {
            Box::pin(async move {
                confirm_identity(transaction, request).await?;
                let permit = transaction.fence_member().await?;
                let t = permit.transaction();
                // RLS shows the member's own rows and those added for everyone.
                let rows = t.query(&format!("SELECT {CARD_COLUMNS},(c.app_id IS NOT NULL AND EXISTS(SELECT 1 FROM genui.apps a WHERE a.organization_id=c.organization_id AND a.app_id=c.app_id AND {DISCOVER} AND {OWNER_OR_ADMIN})) FROM genui.member_cards c WHERE c.organization_id=$1::text::uuid AND c.card_id=$3::text::uuid AND c.removed_at IS NULL"), &[&request.organization,&request.principal,&card]).await?;
                let row = rows.first().ok_or(Error::Denied)?;
                let current = card_row(row, flag(row, 11)?)?;
                if !current.mine {
                    // The steward path: only ever back to its maker.
                    require(!everyone)?;
                    let rows = t.query("SELECT app_id::text,work_id::text,changed FROM genui_private.steward_card($1::text::uuid,$2::text::uuid,$3::text::uuid,false)", &[&request.organization,&request.principal,&card]).await?;
                    let row = rows.first().ok_or(Error::Denied)?;
                    require(flag(row, 2)?)?;
                    let (app, work) = (optional(row, 0)?, optional(row, 1)?);
                    let digest = outcome("shared", &[card, "maker"]);
                    append(t, Chain::Workspace, request, Event { kind: "card.shared", id: None, app: app.as_deref(), work: work.as_deref(), record: None, outcome: Some(&digest) }).await?;
                    return Ok(Carded::Saved(MemberCard {
                        everyone: false,
                        can_manage: false,
                        ..current
                    }));
                }
                // The member's own pinned answer: theirs alone, whoever owns its app.
                if current.kind != "card" {
                    return Ok(Carded::Refused("not_shareable"));
                }
                // Adding for everyone takes the app's owner or an admin;
                // keeping it for oneself takes only being its maker.
                require(current.can_share || !everyone)?;
                let app = current.app.clone().ok_or(Error::Denied)?;
                if current.everyone == everyone {
                    return Ok(Carded::Unchanged(current));
                }
                let written = t.query("UPDATE genui.member_cards SET everyone=$4,updated_at=date_trunc('second',now()) WHERE organization_id=$1::text::uuid AND card_id=$3::text::uuid AND principal_id=$2::text::uuid AND removed_at IS NULL RETURNING 1", &[&request.organization,&request.principal,&card,&everyone]).await?;
                require(written.len() == 1)?;
                let digest = outcome("shared", &[card, if everyone { "everyone" } else { "mine" }]);
                append(t, Chain::Workspace, request, Event { kind: "card.shared", id: None, app: Some(&app), work: current.work.as_deref(), record: None, outcome: Some(&digest) }).await?;
                Ok(Carded::Saved(MemberCard { everyone, ..current }))
            })
        })
        .await
    }

    /// Removes the member's own card or pinned answer (a soft delete; one
    /// added for everyone goes for everyone): its maker, or (the steward
    /// path, `genui_private.steward_card`) the app's owner or an admin who
    /// can open it for another member's card added for everyone, else
    /// `Denied`. Audited `card.removed`; removing it again writes nothing.
    pub async fn remove_card(
        &self,
        request: &TrustedRequest,
        card: &str,
    ) -> Result<Carded<String>> {
        require(valid_id(card))?;
        self.cards_transaction(request, |transaction| {
            Box::pin(async move {
                confirm_identity(transaction, request).await?;
                let permit = transaction.fence_member().await?;
                let t = permit.transaction();
                // RLS shows the member's own rows and those added for everyone.
                let rows = t.query("SELECT app_id::text,work_id::text,removed_at IS NULL,principal_id=$2::text::uuid FROM genui.member_cards WHERE organization_id=$1::text::uuid AND card_id=$3::text::uuid", &[&request.organization,&request.principal,&card]).await?;
                let row = rows.first().ok_or(Error::Denied)?;
                let (mut app, mut work) = (optional(row, 0)?, optional(row, 1)?);
                if flag(row, 3)? {
                    if !flag(row, 2)? {
                        return Ok(Carded::Unchanged(card.to_owned()));
                    }
                    let written = t.query("UPDATE genui.member_cards SET removed_at=date_trunc('second',now()),updated_at=date_trunc('second',now()) WHERE organization_id=$1::text::uuid AND card_id=$3::text::uuid AND principal_id=$2::text::uuid AND removed_at IS NULL RETURNING 1", &[&request.organization,&request.principal,&card]).await?;
                    require(written.len() == 1)?;
                } else {
                    let rows = t.query("SELECT app_id::text,work_id::text,changed FROM genui_private.steward_card($1::text::uuid,$2::text::uuid,$3::text::uuid,true)", &[&request.organization,&request.principal,&card]).await?;
                    let row = rows.first().ok_or(Error::Denied)?;
                    if !flag(row, 2)? {
                        return Ok(Carded::Unchanged(card.to_owned()));
                    }
                    (app, work) = (optional(row, 0)?, optional(row, 1)?);
                }
                let digest = outcome("removed", &[card]);
                append(t, Chain::Workspace, request, Event { kind: "card.removed", id: None, app: app.as_deref(), work: work.as_deref(), record: None, outcome: Some(&digest) }).await?;
                Ok(Carded::Saved(card.to_owned()))
            })
        })
        .await
    }
}

impl Data {
    /// APP-123 (a pinned answer): whether every receipt is one storage
    /// issued, in one `READ ONLY` snapshot under the same member checks and
    /// `record_read` grants as Ask's read (`Data::read_only_listings`): each
    /// names a revision of a record of its work (collection, record,
    /// version, commit sequence) and the publication that revision was read
    /// under (its work's pinned publication, `saved_work.publication_id`,
    /// which never changes) with that definition digest; another
    /// publication of the same app is refused (review finding 7). A work
    /// the member cannot read is `Denied` (the route's uniform 404).
    pub async fn receipts_known(
        &self,
        request: &TrustedRequest,
        works: &[(&str, &str)],
        receipts: &[ReceiptKey],
    ) -> Result<bool> {
        require(
            (1..=super::READ_ONLY_WORKS).contains(&works.len())
                && works
                    .iter()
                    .all(|(app, work)| valid_id(app) && valid_id(work))
                && receipts.iter().all(|r| {
                    works.contains(&(r.app.as_str(), r.work.as_str()))
                        && valid_id(&r.collection)
                        && valid_id(&r.record)
                        && valid_id(&r.publication)
                }),
        )?;
        let cap = Instant::now() + QUERY_HANDLER_DEADLINE;
        let transaction = Transaction::begin_read_only(&self.inner, request, cap).await?;
        let result = async {
            let grants = transaction.read_only_grants(works).await?;
            require(grants.len() == works.len() && grants.iter().all(|grant| grant.record_read))?;
            if receipts.is_empty() {
                return Ok(true);
            }
            let column = |f: fn(&ReceiptKey) -> &str| -> Vec<&str> { receipts.iter().map(f).collect() };
            let (apps, works_, collections, records, publications, digests) = (
                column(|r| &r.app),
                column(|r| &r.work),
                column(|r| &r.collection),
                column(|r| &r.record),
                column(|r| &r.publication),
                column(|r| &r.definition_digest),
            );
            let versions: Vec<i64> = receipts.iter().map(|r| r.version).collect();
            let sequences: Vec<i64> = receipts.iter().map(|r| r.commit_sequence).collect();
            let rows = transaction.query("SELECT count(*) FROM unnest($2::text[],$3::text[],$4::text[],$5::text[],$6::bigint[],$7::bigint[],$8::text[],$9::text[]) AS k(app,work,collection,record,version,seq,publication,digest) WHERE EXISTS(SELECT 1 FROM genui.internal_record_revisions v WHERE v.organization_id=$1::text::uuid AND v.app_id=k.app::uuid AND v.work_id=k.work::uuid AND v.collection_id=k.collection::uuid AND v.record_id=k.record::uuid AND v.version=k.version AND v.commit_sequence=k.seq) AND EXISTS(SELECT 1 FROM genui.saved_work w JOIN genui.publications p ON p.organization_id=w.organization_id AND p.app_id=w.app_id AND p.publication_id=w.publication_id WHERE w.organization_id=$1::text::uuid AND w.app_id=k.app::uuid AND w.work_id=k.work::uuid AND p.publication_id=k.publication::uuid AND p.definition_digest::text=k.digest)", &[&request.organization,&apps,&works_,&collections,&records,&versions,&sequences,&publications,&digests]).await?;
            let found: i64 = rows.first().ok_or(Error::Unavailable)?.try_get(0).map_err(|_| Error::Unavailable)?;
            Ok(usize::try_from(found).is_ok_and(|found| found == receipts.len()))
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_outcomes_are_domain_separated() {
        assert_ne!(outcome("card", &["a"]), outcome("answer", &["a"]));
        assert_ne!(
            outcome("shared", &["a", "everyone"]),
            outcome("shared", &["a", "mine"])
        );
        assert!(outcome("removed", &["a"]).starts_with("sha256:"));
        assert_eq!(spec_digest(b"{}").len(), 64);
    }
}
