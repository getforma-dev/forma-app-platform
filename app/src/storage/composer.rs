//! APP-093 composer context (ADR-011 §3 I1, I2, I5; ADR-007 D7.3 and its
//! 2026-10-29 creator-authority amendment).
//!
//! One READ COMMITTED workspace transaction under the member fence
//! (`fence_member`, as the APP-076 workspace listing) reads everything the
//! composer needs and nothing it may not: the app the member **owns** with
//! `edit_definition` and `run` (where a generated or adapted definition is
//! drafted and owner-published; creating an app and granting on it stay
//! operator actions, so a member without such an app cannot generate), the
//! organization's newest `owner_publish` policy, the live source revisions
//! the member holds `source_read` on (each as its typed input contract), and
//! the library: every app of the organization with a publication the member
//! may `discover` and `run`, read from its newest publication's stored
//! definition re-admitted (`stored_content`), kept only when every published
//! input can be bound for this member now (a source input to a readable
//! revision carrying the same contract digest, a records input to a record
//! schema with the same fields). A foreign organization's rows are
//! unreachable (RLS); an item the member may not reach is omitted, never
//! named. Nothing here writes: drafting, publishing and saving go through the
//! APP-010 operations unchanged, each with its own precheck and fence.
use super::{
    authorization::{Action, Resource, Transaction},
    definitions::{confirm_identity, stored_content},
    pool::Workspace,
    Error, Result, TrustedRequest, WorkBinding, MAX_CANONICAL_BYTES,
};
use crate::admission::{self, digest};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};
use tokio::time::Instant;
use tokio_postgres::Row;

/// ADR-007 D7.14 handler deadline for this one read.
pub const CONTEXT_DEADLINE: Duration = Duration::from_secs(5);
/// Library apps considered (the pitch library is 10–14 apps).
const LIBRARY_CANDIDATES: i64 = 24;
/// Live source revisions considered.
const SOURCE_CANDIDATES: i64 = 64;

/// The app a generated or adapted definition is drafted on: owned by the
/// member, who holds `edit_definition` (draft, owner-publish) and `run`
/// (save work) on it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoringApp {
    pub app: String,
}

/// An app the member owns, for the authoring choice (`choose_authoring`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoringCandidate {
    pub app: String,
    /// The definition digest of its newest publication; `None`: never published.
    pub newest: Option<String>,
    /// The member holds `edit_definition` and `run` on it.
    pub granted: bool,
}

/// The authoring app among the member's owned apps (in id order): the first
/// granted app never published, else the first granted app whose newest
/// publication is not a library definition (`sidecar`); never a library app.
pub fn choose_authoring(
    candidates: &[AuthoringCandidate],
    sidecar: &BTreeSet<String>,
) -> Option<String> {
    let granted = || candidates.iter().filter(|c| c.granted);
    granted()
        .find(|c| c.newest.is_none())
        .or_else(|| {
            granted().find(|c| {
                c.newest
                    .as_ref()
                    .is_some_and(|digest| !sidecar.contains(digest))
            })
        })
        .map(|c| c.app.clone())
}

/// One live source revision the member may read, as the typed input
/// contract its envelope carries (contract edition impl-2026-10-28.1 shape:
/// `id`, `contract_id`, `contract_version`, `kind`, `fields`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadableSource {
    pub source: String,
    pub revision: String,
    /// `forma:input-contract:v1` identity of `contract`.
    pub contract_digest: String,
    pub contract: Value,
}

/// One published input of a library definition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedInput {
    pub id: String,
    /// `source` or `records`.
    pub kind: String,
    pub contract_digest: String,
    /// The typed `fields` array (a records input binds the record schema
    /// with these fields).
    pub fields: Value,
}

/// One view of a library definition, for the router's catalog.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibraryView {
    pub id: String,
    pub kind: String,
    pub title: String,
}

/// One library app the member may instantiate now.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibraryApp {
    pub app: String,
    pub publication: String,
    pub definition_digest: String,
    pub title: String,
    pub description: String,
    pub inputs: Vec<PublishedInput>,
    /// `(schema_id, fields)` of every record schema.
    pub record_schemas: Vec<(String, Value)>,
    pub views: Vec<LibraryView>,
    /// The definition content, re-admitted (for the adapted path's typed
    /// patch, which is re-admitted whole again).
    pub content: Value,
}

/// Everything the composer may act on for this member, read once.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ComposerContext {
    pub authoring: Option<AuthoringApp>,
    /// The organization's newest `owner_publish` policy id.
    pub owner_publish_policy: Option<String>,
    pub sources: Vec<ReadableSource>,
    pub library: Vec<LibraryApp>,
}

fn text(row: &Row, index: usize) -> Result<String> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}
fn bytes(row: &Row, index: usize) -> Result<Vec<u8>> {
    let value: Vec<u8> = row.try_get(index).map_err(|_| Error::Unavailable)?;
    if value.len() > MAX_CANONICAL_BYTES {
        return Err(Error::Unavailable);
    }
    Ok(value)
}
/// `Ok(false)` for a denial (the item is omitted); a store failure ends the
/// read.
fn granted(result: Result<()>) -> Result<bool> {
    match result {
        Ok(()) => Ok(true),
        Err(Error::Denied) => Ok(false),
        Err(error) => Err(error),
    }
}
fn string(value: Option<&Value>) -> Result<String> {
    value
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or(Error::Unavailable)
}

/// The published inputs, record schemas `(id, fields)` and views of one
/// definition content.
type Summary = (Vec<PublishedInput>, Vec<(String, Value)>, Vec<LibraryView>);

/// The published inputs, record schemas and views of re-admitted content.
fn summarize(content: &Value) -> Result<Summary> {
    let items = |key: &str| -> Result<&Vec<Value>> {
        content
            .get(key)
            .and_then(Value::as_array)
            .ok_or(Error::Unavailable)
    };
    let mut inputs = Vec::new();
    for input in items("inputs")? {
        let canonical = admission::encode(input).map_err(|_| Error::Unavailable)?;
        inputs.push(PublishedInput {
            id: string(input.get("id"))?,
            kind: string(input.get("kind"))?,
            contract_digest: digest::digest(digest::INPUT_CONTRACT, &canonical),
            fields: input.get("fields").cloned().ok_or(Error::Unavailable)?,
        });
    }
    let mut schemas = Vec::new();
    for schema in items("record_schemas")? {
        schemas.push((
            string(schema.get("id"))?,
            schema.get("fields").cloned().ok_or(Error::Unavailable)?,
        ));
    }
    let mut views = Vec::new();
    for view in items("views")? {
        views.push(LibraryView {
            id: string(view.get("id"))?,
            kind: string(view.get("kind"))?,
            title: string(view.get("title"))?,
        });
    }
    Ok((inputs, schemas, views))
}

/// The saved-work bindings that instantiate `inputs` for a member who may
/// read `sources`: each source input to the first readable revision carrying
/// its contract digest, each records input to the record schema with its
/// fields. `None` when any input cannot be bound (the app is not offered).
pub fn bindings(
    inputs: &[PublishedInput],
    record_schemas: &[(String, Value)],
    sources: &[ReadableSource],
) -> Option<Vec<WorkBinding>> {
    let mut bound = Vec::with_capacity(inputs.len());
    for input in inputs {
        bound.push(match input.kind.as_str() {
            "source" => {
                let source = sources
                    .iter()
                    .find(|source| source.contract_digest == input.contract_digest)?;
                WorkBinding::Source {
                    input: input.id.clone(),
                    source: source.source.clone(),
                    revision: source.revision.clone(),
                }
            }
            "records" => {
                let (schema, _) = record_schemas
                    .iter()
                    .find(|(_, fields)| *fields == input.fields)?;
                WorkBinding::Records {
                    input: input.id.clone(),
                    schema: schema.clone(),
                }
            }
            _ => return None,
        });
    }
    Some(bound)
}

impl Workspace {
    /// Reads the member's composer context (module documentation). Every
    /// refusal is `Denied` (the route answers the uniform 404); a store
    /// failure is `Unavailable`.
    pub async fn composer_context(&self, request: &TrustedRequest) -> Result<ComposerContext> {
        let cap = Instant::now() + CONTEXT_DEADLINE;
        let mut transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = async {
            confirm_identity(&transaction, request).await?;
            let permit = transaction.fence_member().await?;
            let org = &request.organization;
            let principal = &request.principal;

            // The authoring app: owned, with edit_definition and run, and
            // never a library app (APP-093 review M2: a generated revision
            // published onto a library app displaces that library entry in
            // this catalog). An owned app that has never been published (the
            // dedicated authoring shell `grant-library-access` creates) is
            // preferred; then one whose newest publication is not a library
            // definition; a library app is never chosen.
            let sidecar: BTreeSet<String> = super::queries::library_sidecar()?
                .iter()
                .filter_map(|entry| entry.get("definition_digest")?.as_str().map(str::to_owned))
                .collect();
            let mut candidates = Vec::new();
            let owned = permit.transaction().query("SELECT a.app_id::text, (SELECT p.definition_digest::text FROM genui.publications p WHERE p.organization_id=a.organization_id AND p.app_id=a.app_id ORDER BY p.publication_sequence DESC LIMIT 1) FROM genui.apps a WHERE a.organization_id=$1::text::uuid AND a.owner_principal_id=$2::text::uuid ORDER BY a.app_id", &[org,principal]).await?;
            for row in &owned {
                let app = text(row, 0)?;
                let newest: Option<String> = row.try_get(1).map_err(|_| Error::Unavailable)?;
                let granted = granted(permit.also(Action::EditDefinition, Resource::App(&app)).await)?
                    && granted(permit.also(Action::Run, Resource::App(&app)).await)?;
                candidates.push(AuthoringCandidate { app, newest, granted });
            }
            let authoring = choose_authoring(&candidates, &sidecar).map(|app| AuthoringApp { app });

            let policy = permit.transaction().query("SELECT policy_id::text FROM genui.publication_policies WHERE organization_id=$1::text::uuid AND kind='owner_publish' ORDER BY policy_version DESC,policy_id LIMIT 1", &[org]).await?;
            let owner_publish_policy = match policy.first() {
                Some(row) => Some(text(row, 0)?),
                None => None,
            };

            // Readable live source revisions, as typed input contracts.
            let mut sources = Vec::new();
            let mut readable: BTreeMap<String, bool> = BTreeMap::new();
            let revisions = permit.transaction().query("SELECT source_id::text,revision_id::text,contract_digest::text,canonical_envelope FROM genui.source_revisions WHERE organization_id=$1::text::uuid AND deleted_at IS NULL ORDER BY source_id,revision_id LIMIT $2", &[org,&SOURCE_CANDIDATES]).await?;
            for row in &revisions {
                let source = text(row, 0)?;
                let allowed = match readable.get(&source) {
                    Some(allowed) => *allowed,
                    None => {
                        let allowed = granted(
                            permit
                                .also(Action::SourceRead, Resource::Source(&source))
                                .await,
                        )?;
                        readable.insert(source.clone(), allowed);
                        allowed
                    }
                };
                if !allowed {
                    continue;
                }
                let envelope =
                    admission::decode(&bytes(row, 3)?).map_err(|_| Error::Unavailable)?;
                let contract = envelope
                    .get("input_contract")
                    .cloned()
                    .ok_or(Error::Unavailable)?;
                let canonical = admission::encode(&contract).map_err(|_| Error::Unavailable)?;
                let contract_digest = digest::digest(digest::INPUT_CONTRACT, &canonical);
                // Stored state that contradicts itself is unavailability.
                if contract_digest != text(row, 2)? {
                    return Err(Error::Unavailable);
                }
                sources.push(ReadableSource {
                    source,
                    revision: text(row, 1)?,
                    contract_digest,
                    contract,
                });
            }

            // The library: newest publication per app, discover and run.
            let mut library = Vec::new();
            let publications = permit.transaction().query("SELECT DISTINCT ON (app_id) app_id::text,publication_id::text,definition_revision_id::text,definition_digest::text FROM genui.publications WHERE organization_id=$1::text::uuid ORDER BY app_id,publication_sequence DESC LIMIT $2", &[org,&LIBRARY_CANDIDATES]).await?;
            for row in &publications {
                let app = text(row, 0)?;
                if !granted(permit.also(Action::Discover, Resource::App(&app)).await)?
                    || !granted(permit.also(Action::Run, Resource::App(&app)).await)?
                {
                    continue;
                }
                let revision = text(row, 2)?;
                let (admitted, canonical) =
                    stored_content(permit.transaction(), org, &app, &revision).await?;
                if admitted != text(row, 3)? {
                    return Err(Error::Unavailable);
                }
                let content = admission::decode(&canonical).map_err(|_| Error::Unavailable)?;
                let (inputs, record_schemas, views) = summarize(&content)?;
                if bindings(&inputs, &record_schemas, &sources).is_none() {
                    continue;
                }
                library.push(LibraryApp {
                    app,
                    publication: text(row, 1)?,
                    definition_digest: admitted,
                    title: string(content.get("title"))?,
                    description: string(content.get("description"))?,
                    inputs,
                    record_schemas,
                    views,
                    content,
                });
            }
            Ok(ComposerContext {
                authoring,
                owner_publish_policy,
                sources,
                library,
            })
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn source(id: &str, digest: &str) -> ReadableSource {
        ReadableSource {
            source: id.into(),
            revision: format!("r-{id}"),
            contract_digest: digest.into(),
            contract: json!({}),
        }
    }

    #[test]
    fn bindings_match_sources_by_contract_digest_and_records_by_fields() {
        let fields = json!([{"id": "status", "nullable": false, "value_type": {"type": "text"}}]);
        let inputs = vec![
            PublishedInput {
                id: "events".into(),
                kind: "source".into(),
                contract_digest: "sha256:aa".into(),
                fields: json!([]),
            },
            PublishedInput {
                id: "decisions".into(),
                kind: "records".into(),
                contract_digest: "sha256:bb".into(),
                fields: fields.clone(),
            },
        ];
        let schemas = vec![("decision".to_owned(), fields)];
        let sources = [source("s1", "sha256:zz"), source("s2", "sha256:aa")];
        assert_eq!(
            bindings(&inputs, &schemas, &sources),
            Some(vec![
                WorkBinding::Source {
                    input: "events".into(),
                    source: "s2".into(),
                    revision: "r-s2".into(),
                },
                WorkBinding::Records {
                    input: "decisions".into(),
                    schema: "decision".into(),
                },
            ])
        );
        // No readable revision with that contract: the app is not offered.
        assert_eq!(bindings(&inputs, &schemas, &sources[..1]), None);
        // A records input without a schema of its fields: not offered.
        assert_eq!(bindings(&inputs, &[], &sources), None);
    }

    #[test]
    fn summarize_reads_inputs_schemas_and_views() {
        let content = json!({
            "title": "t", "description": "d",
            "inputs": [{"id": "events", "contract_id": "events_v1", "contract_version": 1, "kind": "source",
                "fields": [{"id": "bytes", "value_type": {"type": "integer", "unit": "byte"}, "nullable": false}]}],
            "record_schemas": [{"id": "decision", "version": 1, "fields": [{"id": "note", "value_type": {"type": "text"}, "nullable": false}]}],
            "views": [{"id": "total", "kind": "summary", "renderer_version": 1, "title": "Total"}],
        });
        let (inputs, schemas, views) = summarize(&content).unwrap();
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].id, "events");
        assert_eq!(inputs[0].kind, "source");
        assert!(inputs[0].contract_digest.starts_with("sha256:"));
        assert_eq!(schemas[0].0, "decision");
        assert_eq!(
            views,
            vec![LibraryView {
                id: "total".into(),
                kind: "summary".into(),
                title: "Total".into(),
            }]
        );
        assert_eq!(summarize(&json!({"title": "t"})), Err(Error::Unavailable));
    }
}
