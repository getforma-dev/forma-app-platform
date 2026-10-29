use super::{
    pool::{Control, Data, Lease, Role, RolePool, Workspace},
    require, valid_id, Error, Result,
};
use std::sync::Arc;
use tokio::time::{timeout_at, Instant};
use tokio_postgres::{error::SqlState, types::ToSql, Row};

/// No public constructor, deserializer or clone. APP-009 must supply the
/// reviewed session adapter before ordinary production access.
///
/// ```compile_fail
/// use genui::storage::TrustedRequest;
/// let request = TrustedRequest {
///     organization: String::new(), principal: String::new(), epoch: 1,
///     issuer: String::new(), provider_tenant: String::new(),
///     provider_subject: String::new(), registry_sha256: String::new(),
/// };
/// ```
pub struct TrustedRequest {
    pub(super) organization: String,
    pub(super) principal: String,
    pub(super) epoch: i64,
    pub(super) issuer: String,
    pub(super) provider_tenant: String,
    pub(super) provider_subject: String,
    pub(super) registry_sha256: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Discover,
    Run,
    SourceRead,
    RecordRead,
    RecordWrite,
    EditDefinition,
    ReviewPublish,
    ShareWork,
    Export,
    AdminPolicy,
}
impl TryFrom<&str> for Action {
    type Error = Error;
    fn try_from(value: &str) -> Result<Self> {
        Ok(match value {
            "discover" => Self::Discover,
            "run" => Self::Run,
            "source_read" => Self::SourceRead,
            "record_read" => Self::RecordRead,
            "record_write" => Self::RecordWrite,
            "edit_definition" => Self::EditDefinition,
            "review_publish" => Self::ReviewPublish,
            "share_work" => Self::ShareWork,
            "export" => Self::Export,
            "admin_policy" => Self::AdminPolicy,
            _ => return Err(Error::Denied),
        })
    }
}
impl Action {
    fn permission(self) -> &'static str {
        match self {
            Self::Discover => "discover",
            Self::Run => "run",
            Self::SourceRead => "source_read",
            Self::RecordRead => "record_read",
            Self::RecordWrite => "record_write",
            Self::EditDefinition => "edit_definition",
            Self::ReviewPublish => "review_publish",
            Self::ShareWork => "share_work",
            Self::Export => "export",
            Self::AdminPolicy => "admin_policy",
        }
    }
}

// These scoped selectors and permits are private; no arbitrary SQL/client,
// transaction, row stream or CancelToken can escape a storage operation.
#[cfg_attr(not(test), allow(dead_code))] // Closed permission variants reserved for later typed operations.
pub(super) enum Resource<'a> {
    Organization,
    App(&'a str),
    Work { app: &'a str, work: &'a str },
    Source(&'a str),
}
pub(super) struct Precheck<'a> {
    action: Action,
    resource: Resource<'a>,
}
pub(super) struct Permit<'a, 'r> {
    transaction: &'a mut Transaction<'r>,
    request: &'r TrustedRequest,
    precheck: Precheck<'r>,
}

/// A member transaction carries its trusted request; the nonmember entry
/// (ADR-007 D7.1, admission only) carries none and can never precheck, fence
/// or obtain a `Permit`.
pub(super) struct Transaction<'a> {
    pub lease: Lease,
    pub deadline: Instant,
    request: Option<&'a TrustedRequest>,
}
impl<'a> Transaction<'a> {
    pub async fn begin(pool: &Arc<RolePool>, request: &'a TrustedRequest) -> Result<Self> {
        require(
            valid_id(&request.organization)
                && valid_id(&request.principal)
                && (1..=9_007_199_254_740_991).contains(&request.epoch),
        )?;
        require(
            pool.registry.identity.sha256 == request.registry_sha256
                && pool.registry.agrees(
                    &request.issuer,
                    &request.provider_tenant,
                    &request.organization,
                ),
        )?;
        Self::open(
            pool,
            Some(request),
            &request.organization,
            &request.principal,
            &request.epoch.to_string(),
        )
        .await
    }
    /// ADR-007 D7.1 private nonmember entry for the admission definer calls:
    /// workspace pool only, organization scope (or none, for the two pre-scope
    /// lookups) and no principal or epoch. It never issues a member `Permit`.
    /// Call it only from `storage::admission`: the type system admits any
    /// `storage` module, so that restriction is this convention (review nit).
    pub async fn begin_nonmember(pool: &Arc<RolePool>, organization: Option<&str>) -> Result<Self> {
        require(pool.role() == Role::Workspace && organization.is_none_or(valid_id))?;
        Self::open(pool, None, organization.unwrap_or(""), "", "").await
    }
    async fn open(
        pool: &Arc<RolePool>,
        request: Option<&'a TrustedRequest>,
        organization: &str,
        principal: &str,
        epoch: &str,
    ) -> Result<Self> {
        let lease = Lease::acquire(pool).await?;
        let deadline = lease.transaction_deadline()?;
        let transaction = Self {
            lease,
            deadline,
            request,
        };
        transaction
            .batch("BEGIN ISOLATION LEVEL READ COMMITTED READ WRITE")
            .await?;
        #[cfg(test)]
        transaction.lease.checkpoint("begin").await;
        transaction.query("SELECT set_config('genui.organization_id',$1,true),set_config('genui.principal_id',$2,true),set_config('genui.membership_epoch',$3,true)",
            &[&organization, &principal, &epoch]).await?;
        Ok(transaction)
    }
    fn member(&self) -> Result<&'a TrustedRequest> {
        self.request.ok_or(Error::Denied)
    }
    pub async fn batch(&self, sql: &str) -> Result<()> {
        timeout_at(self.deadline, self.lease.client().batch_execute(sql))
            .await
            .map_err(|_| Error::Deadline)?
            .map_err(|_| Error::Unavailable)
    }
    pub async fn query(&self, sql: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Vec<Row>> {
        timeout_at(self.deadline, self.lease.client().query(sql, params))
            .await
            .map_err(|_| Error::Deadline)?
            .map_err(|_| Error::Unavailable)
    }
    /// As `query`, but a database refusal keeps its SQLSTATE so that the
    /// storage module can classify it (ADR-007 D7.9). Nothing leaves storage.
    pub async fn query_classified(
        &self,
        sql: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<std::result::Result<Vec<Row>, SqlState>> {
        match timeout_at(self.deadline, self.lease.client().query(sql, params))
            .await
            .map_err(|_| Error::Deadline)?
        {
            Ok(rows) => Ok(Ok(rows)),
            Err(error) => error.code().cloned().map(Err).ok_or(Error::Unavailable),
        }
    }
    pub async fn precheck(&self, action: Action, resource: Resource<'a>) -> Result<Precheck<'a>> {
        let request = self.member()?;
        require(self.lease.role() != Role::Worker)?;
        let rows = self.query("SELECT o.status,m.status,m.membership_epoch FROM genui.organizations o JOIN genui.memberships m USING(organization_id) WHERE o.organization_id=$1::text::uuid AND m.principal_id=$2::text::uuid", &[&request.organization,&request.principal]).await?;
        let row = rows.first().ok_or(Error::Denied)?;
        require(
            text(row, 0)? == "active"
                && text(row, 1)? == "active"
                && integer(row, 2)? == request.epoch,
        )?;
        self.entitlement(request).await?;
        let precheck = Precheck { action, resource };
        self.permissions(request, &precheck).await?;
        Ok(precheck)
    }
    /// APP-073 (ADR-003, ADR-004): every member action also needs a current
    /// trial entitlement, judged on the wall clock of this statement so that
    /// an entitlement expiring inside the transaction is not honored later.
    async fn entitlement(&self, request: &TrustedRequest) -> Result<()> {
        let rows = self.query("SELECT 1 FROM genui.entitlements WHERE organization_id=$1::text::uuid AND principal_id=$2::text::uuid AND status='active' AND expires_at>clock_timestamp() LIMIT 1", &[&request.organization,&request.principal]).await?;
        require(!rows.is_empty())
    }
    async fn permissions(&self, request: &TrustedRequest, precheck: &Precheck<'_>) -> Result<()> {
        let org = &request.organization;
        let principal = &request.principal;
        let action = precheck.action.permission();
        let rows = match precheck.resource {
            Resource::Organization if precheck.action == Action::AdminPolicy => self.query(
                "SELECT 1 FROM genui.organization_permissions WHERE organization_id=$1::text::uuid AND principal_id=$2::text::uuid AND permission=$3", &[org,principal,&action]).await?,
            Resource::App(app) if matches!(precheck.action, Action::Discover | Action::Run | Action::EditDefinition | Action::ReviewPublish) => {
                require(valid_id(app))?;
                self.query("SELECT 1 FROM genui.app_permissions p JOIN genui.apps a USING(organization_id,app_id) WHERE p.organization_id=$1::text::uuid AND p.principal_id=$2::text::uuid AND p.app_id=$3::text::uuid AND p.permission=$4", &[org,principal,&app,&action]).await?
            }
            Resource::Source(source) if precheck.action == Action::SourceRead => {
                require(valid_id(source))?;
                self.query("SELECT 1 FROM genui.source_permissions p JOIN genui.sources s USING(organization_id,source_id) WHERE p.organization_id=$1::text::uuid AND p.principal_id=$2::text::uuid AND p.source_id=$3::text::uuid AND p.permission='source_read'", &[org,principal,&source]).await?
            }
            Resource::Work { app, work } if matches!(precheck.action, Action::RecordRead | Action::RecordWrite | Action::ShareWork | Action::Export) => {
                require(valid_id(app) && valid_id(work))?;
                self.query("SELECT 1 FROM genui.work_permissions p JOIN genui.saved_work w USING(organization_id,app_id,work_id) WHERE p.organization_id=$1::text::uuid AND p.principal_id=$2::text::uuid AND p.app_id=$3::text::uuid AND p.work_id=$4::text::uuid AND p.permission=$5 AND EXISTS(SELECT 1 FROM genui.app_permissions a WHERE a.organization_id=p.organization_id AND a.principal_id=p.principal_id AND a.app_id=p.app_id AND a.permission='run') AND NOT EXISTS(SELECT 1 FROM genui.work_source_dependencies d JOIN genui.source_revisions r ON r.organization_id=d.organization_id AND r.source_id=d.source_id AND r.revision_id=d.source_revision_id AND r.normalized_digest=d.source_digest WHERE d.organization_id=p.organization_id AND d.app_id=p.app_id AND d.work_id=p.work_id AND (r.deleted_at IS NOT NULL OR NOT EXISTS(SELECT 1 FROM genui.source_permissions s WHERE s.organization_id=d.organization_id AND s.principal_id=p.principal_id AND s.source_id=d.source_id AND s.permission='source_read')))", &[org,principal,&app,&work,&action]).await?
            }
            _ => return Err(Error::Denied),
        };
        require(!rows.is_empty())
    }
    pub async fn fence<'t>(&'t mut self, precheck: Precheck<'a>) -> Result<Permit<'t, 'a>> {
        let request = self.member()?;
        let rows = self.query("SELECT organization_active,member_active,membership_epoch FROM genui_private.lock_member_authority($1::text::uuid,$2::text::uuid,$3)", &[&request.organization,&request.principal,&request.epoch]).await?;
        let row = rows.first().ok_or(Error::Denied)?;
        require(boolean(row, 0)? && boolean(row, 1)? && integer(row, 2)? == request.epoch)?;
        // Entitlement revocation advances the epoch under the member lock
        // (operator channel), so this re-read after the fence also observes
        // expiry that happened since the precheck.
        self.entitlement(request).await?;
        self.permissions(request, &precheck).await?;
        Ok(Permit {
            transaction: self,
            request,
            precheck,
        })
    }
    pub async fn finish(self, commit: bool) -> Result<()> {
        self.lease.finish(commit, self.deadline).await
    }
}

fn text(row: &Row, index: usize) -> Result<String> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}
fn integer(row: &Row, index: usize) -> Result<i64> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}
fn boolean(row: &Row, index: usize) -> Result<bool> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}

#[derive(Debug, Eq, PartialEq)]
pub struct Publication {
    pub publication_id: String,
    pub definition_digest: String,
}
#[derive(Debug, Eq, PartialEq)]
pub struct Record {
    pub record_id: String,
    pub version: i64,
}

impl Permit<'_, '_> {
    async fn publication(&self, publication: &str) -> Result<Publication> {
        require(valid_id(publication))?;
        let Resource::App(app) = self.precheck.resource else {
            return Err(Error::Denied);
        };
        let rows = self.transaction.query("SELECT publication_id::text,definition_digest::text FROM genui.publications WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND publication_id=$3::text::uuid", &[&self.request.organization,&app,&publication]).await?;
        let row = rows.first().ok_or(Error::Denied)?;
        Ok(Publication {
            publication_id: text(row, 0)?,
            definition_digest: text(row, 1)?,
        })
    }
    async fn record(&self, collection: &str, record: &str) -> Result<Record> {
        require(
            self.transaction.lease.role() == Role::Data
                && self.precheck.action == Action::RecordRead
                && valid_id(collection)
                && valid_id(record),
        )?;
        let Resource::Work { app, work } = self.precheck.resource else {
            return Err(Error::Denied);
        };
        let pin = self.transaction.query("SELECT publication_id::text,version FROM genui.saved_work WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid", &[&self.request.organization,&app,&work]).await?;
        let pin = pin.first().ok_or(Error::Denied)?;
        let publication = text(pin, 0)?;
        let version = integer(pin, 1)?;
        let locked = self.transaction.query("SELECT publication_id::text,work_version FROM genui_private.lock_work_revision($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5)", &[&self.request.organization,&app,&work,&publication,&version]).await?;
        let locked = locked.first().ok_or(Error::Denied)?;
        require(text(locked, 0)? == publication && integer(locked, 1)? == version)?;
        let rows = self.transaction.query("SELECT record_id::text,current_version FROM genui.internal_records WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid AND collection_id=$4::text::uuid AND record_id=$5::text::uuid", &[&self.request.organization,&app,&work,&collection,&record]).await?;
        let row = rows.first().ok_or(Error::Denied)?;
        Ok(Record {
            record_id: text(row, 0)?,
            version: integer(row, 1)?,
        })
    }
}

async fn publication(
    pool: &Arc<RolePool>,
    request: &TrustedRequest,
    app: &str,
    id: &str,
    action: Action,
) -> Result<Publication> {
    let mut transaction = Transaction::begin(pool, request).await?;
    let result = async {
        if transaction.lease.role() == Role::Workspace {
            let confirmed = transaction.query("SELECT 1 FROM genui.principals p JOIN genui.provider_tenant_bindings b USING(organization_id,issuer,provider_tenant_id) WHERE p.organization_id=$1::text::uuid AND p.principal_id=$2::text::uuid AND p.issuer=$3 AND p.provider_tenant_id=$4 AND p.provider_subject=$5 AND b.active", &[&request.organization,&request.principal,&request.issuer,&request.provider_tenant,&request.provider_subject]).await?;
            require(confirmed.len() == 1)?;
        }
        let precheck = transaction.precheck(action,Resource::App(app)).await?;
        transaction.fence(precheck).await?.publication(id).await
    }.await;
    transaction.finish(result.is_ok()).await?;
    result
}
impl Workspace {
    pub async fn read_publication(
        &self,
        request: &TrustedRequest,
        app: &str,
        id: &str,
    ) -> Result<Publication> {
        publication(&self.inner, request, app, id, Action::Discover).await
    }
}
impl Control {
    pub async fn read_publication(
        &self,
        request: &TrustedRequest,
        app: &str,
        id: &str,
    ) -> Result<Publication> {
        publication(&self.inner, request, app, id, Action::ReviewPublish).await
    }
}
impl Data {
    pub async fn read_record(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
        collection: &str,
        record: &str,
    ) -> Result<Record> {
        let mut transaction = Transaction::begin(&self.inner, request).await?;
        let result = async {
            let precheck = transaction
                .precheck(Action::RecordRead, Resource::Work { app, work })
                .await?;
            transaction
                .fence(precheck)
                .await?
                .record(collection, record)
                .await
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }
}
