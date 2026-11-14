use super::{
    pool::{Control, Data, Lease, Role, RolePool, Workspace},
    require, valid_id, Error, Result,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::time::{timeout_at, Instant};
use tokio_postgres::{error::SqlState, types::ToSql, Row};

/// No public constructor, deserializer or clone. In production only
/// `storage::sessions::resolve_session` (APP-009) constructs one, from a live
/// workspace session row, the member's current epoch and the registry.
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
impl TrustedRequest {
    /// The organization the session resolved to (read-only; for the
    /// structured query log, APP-076 G6). Authority still comes only from the
    /// storage operations that take the whole request.
    pub fn organization(&self) -> &str {
        &self.organization
    }
    /// The member the session resolved to (read-only; APP-095 counts event
    /// streams per member; the composer's per-member rate-limit key and its
    /// structured log, APP-093). Authority still comes only from storage.
    pub fn principal(&self) -> &str {
        &self.principal
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Discover,
    Run,
    SourceRead,
    RecordRead,
    RecordWrite,
    /// APP-191 (D7, D8): a reviewer of the work: `record_read` on it and the
    /// reviewer rule (`reviewer_clause`). Reviewers see and change every row
    /// of an `own` collection and alone run `reviewer_only` commands.
    RecordReview,
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
            "record_review" => Self::RecordReview,
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
            Self::RecordReview => "record_review",
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

/// APP-122: how an Ask or Shape read begins (`Transaction::begin_read_only`).
pub const READ_ONLY_BEGIN: &str = "BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY";

/// A member transaction carries its trusted request; the nonmember entry
/// (ADR-007 D7.1, admission only) carries none and can never precheck, fence
/// or obtain a `Permit`.
pub(super) struct Transaction<'a> {
    pub lease: Lease,
    pub deadline: Instant,
    request: Option<&'a TrustedRequest>,
    /// Set when any statement of this transaction failed with SQLSTATE 40001
    /// (ADR-007 D7.6): PostgreSQL has then aborted the whole transaction.
    serialization: AtomicBool,
    /// Set when any statement failed with SQLSTATE 55P03 (a lock wait past
    /// `lock_timeout`) or 40P01 (a deadlock PostgreSQL resolved against this
    /// transaction): D7.9 answers these as 409 `busy`/`in_flight`, never 503.
    lock_wait: AtomicBool,
}
impl<'a> Transaction<'a> {
    pub async fn begin(pool: &Arc<RolePool>, request: &'a TrustedRequest) -> Result<Self> {
        Self::begin_with(
            pool,
            request,
            "BEGIN ISOLATION LEVEL READ COMMITTED READ WRITE",
            None,
        )
        .await
    }
    /// A bounded bulk initialization only. Ordinary member operations retain
    /// their three-second lease and two-second statement budget.
    pub(super) async fn begin_import(
        pool: &Arc<RolePool>,
        request: &'a TrustedRequest,
    ) -> Result<Self> {
        let mut tx = Self::begin(pool, request).await?;
        tx.deadline = tx.lease.import_deadline()?;
        tx.batch("SET LOCAL statement_timeout=55000; SET LOCAL idle_in_transaction_session_timeout=60000").await?;
        Ok(tx)
    }
    /// As `begin`, with lease acquisition and the transaction deadline both
    /// bounded by `cap` (a caller's handler deadline, D7.14).
    pub async fn begin_until(
        pool: &Arc<RolePool>,
        request: &'a TrustedRequest,
        cap: Instant,
    ) -> Result<Self> {
        Self::begin_with(
            pool,
            request,
            "BEGIN ISOLATION LEVEL READ COMMITTED READ WRITE",
            Some(cap),
        )
        .await
    }
    /// APP-122 (Ask and Shape): a member read that cannot write. One
    /// `REPEATABLE READ READ ONLY` snapshot: the member checks, the grants
    /// and every read see the same committed state, and PostgreSQL refuses
    /// any write (SQLSTATE 25006) and any row lock in it, so no Ask or Shape
    /// path can change a record, a card or an audit row.
    pub async fn begin_read_only(
        pool: &Arc<RolePool>,
        request: &'a TrustedRequest,
        cap: Instant,
    ) -> Result<Self> {
        Self::begin_with(pool, request, READ_ONLY_BEGIN, Some(cap)).await
    }
    /// ADR-007 D7.6 (A1): the control role's publication transaction only.
    /// Everything else stays READ COMMITTED. A 40001 at any statement or at
    /// COMMIT is classified (`serialization_failed`, `commit_serializable`)
    /// so that the caller can retry once in a new transaction. `cap` bounds
    /// lease acquisition and the transaction deadline, so that the attempt and
    /// its retry stay within the D7.14 handler deadline (review m5).
    pub async fn begin_serializable(
        pool: &Arc<RolePool>,
        request: &'a TrustedRequest,
        cap: Instant,
    ) -> Result<Self> {
        require(pool.role() == Role::Control)?;
        Self::begin_with(
            pool,
            request,
            "BEGIN ISOLATION LEVEL SERIALIZABLE READ WRITE",
            Some(cap),
        )
        .await
    }
    async fn begin_with(
        pool: &Arc<RolePool>,
        request: &'a TrustedRequest,
        isolation: &'static str,
        cap: Option<Instant>,
    ) -> Result<Self> {
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
            isolation,
            cap,
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
        Self::open(
            pool,
            None,
            organization.unwrap_or(""),
            "",
            "",
            "BEGIN ISOLATION LEVEL READ COMMITTED READ WRITE",
            None,
        )
        .await
    }
    /// Account-only authentication and exact-work sharing. This never creates a
    /// member Permit. Callers must judge the account session and share before
    /// reading any organization data; only storage's closed sharing modules use it.
    pub(super) async fn begin_shared(
        pool: &Arc<RolePool>,
        organization: Option<&str>,
        account: Option<&str>,
    ) -> Result<Self> {
        require(
            matches!(pool.role(), Role::Workspace | Role::Data)
                && organization.is_none_or(valid_id)
                && account.is_none_or(valid_id),
        )?;
        let transaction = Self::open(
            pool,
            None,
            organization.unwrap_or(""),
            "",
            "",
            "BEGIN ISOLATION LEVEL READ COMMITTED READ WRITE",
            None,
        )
        .await?;
        transaction
            .query(
                "SELECT set_config('genui.account_id',$1,true)",
                &[&account.unwrap_or("")],
            )
            .await?;
        Ok(transaction)
    }
    /// Exact-share record projections use one read-only committed snapshot.
    /// This entry has no member identity and cannot create a member Permit.
    pub(super) async fn begin_shared_read_only(
        pool: &Arc<RolePool>,
        organization: &str,
        account: &str,
    ) -> Result<Self> {
        require(pool.role() == Role::Data && valid_id(organization) && valid_id(account))?;
        let tx = Self::open(
            pool,
            None,
            organization,
            "",
            "",
            READ_ONLY_BEGIN,
            Some(Instant::now() + super::QUERY_HANDLER_DEADLINE),
        )
        .await?;
        tx.query("SELECT set_config('genui.account_id',$1,true)", &[&account])
            .await?;
        Ok(tx)
    }
    async fn open(
        pool: &Arc<RolePool>,
        request: Option<&'a TrustedRequest>,
        organization: &str,
        principal: &str,
        epoch: &str,
        isolation: &'static str,
        cap: Option<Instant>,
    ) -> Result<Self> {
        let (lease, deadline) = match cap {
            None => {
                let lease = Lease::acquire(pool).await?;
                let deadline = lease.transaction_deadline()?;
                (lease, deadline)
            }
            Some(cap) => {
                // Nothing has been sent on a lease that is still being
                // acquired, so abandoning the acquisition at `cap` is safe.
                let lease = timeout_at(cap, Lease::acquire(pool))
                    .await
                    .map_err(|_| Error::Deadline)??;
                let deadline = lease.transaction_deadline()?.min(cap);
                if deadline <= Instant::now() {
                    return Err(Error::Deadline);
                }
                (lease, deadline)
            }
        };
        let transaction = Self {
            lease,
            deadline,
            request,
            serialization: AtomicBool::new(false),
            lock_wait: AtomicBool::new(false),
        };
        #[cfg(test)]
        transaction
            .lease
            .hooks()
            .transactions
            .fetch_add(1, Ordering::SeqCst);
        transaction.batch(isolation).await?;
        #[cfg(test)]
        transaction.lease.checkpoint("begin").await;
        transaction.query("SELECT set_config('genui.organization_id',$1,true),set_config('genui.principal_id',$2,true),set_config('genui.membership_epoch',$3,true)",
            &[&organization, &principal, &epoch]).await?;
        Ok(transaction)
    }
    fn member(&self) -> Result<&'a TrustedRequest> {
        self.request.ok_or(Error::Denied)
    }
    /// Records a 40001, 55P03 or 40P01 refusal; none leaves storage (D7.9).
    fn note(&self, error: &tokio_postgres::Error) {
        match error.code() {
            Some(&SqlState::T_R_SERIALIZATION_FAILURE) => {
                self.serialization.store(true, Ordering::Relaxed);
            }
            Some(&SqlState::LOCK_NOT_AVAILABLE) | Some(&SqlState::T_R_DEADLOCK_DETECTED) => {
                self.lock_wait.store(true, Ordering::Relaxed);
            }
            _ => {}
        }
    }
    /// Whether a statement of this transaction failed with SQLSTATE 40001, so
    /// PostgreSQL has rolled the whole transaction back (D7.6).
    pub fn serialization_failed(&self) -> bool {
        self.serialization.load(Ordering::Relaxed)
    }
    /// Whether a statement of this transaction waited past `lock_timeout`
    /// (55P03) or lost a deadlock (40P01); the transaction is aborted either
    /// way and the caller answers 409 `busy` or `in_flight` (D7.9).
    pub fn lock_timed_out(&self) -> bool {
        self.lock_wait.load(Ordering::Relaxed)
    }
    #[cfg(test)]
    fn counted(&self) {
        self.lease.hooks().statements.fetch_add(1, Ordering::SeqCst);
    }
    #[cfg(not(test))]
    fn counted(&self) {}
    pub async fn batch(&self, sql: &str) -> Result<()> {
        self.counted();
        timeout_at(self.deadline, self.lease.client().batch_execute(sql))
            .await
            .map_err(|_| Error::Deadline)?
            .map_err(|error| {
                self.note(&error);
                Error::Unavailable
            })
    }
    pub async fn query(&self, sql: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Vec<Row>> {
        self.counted();
        timeout_at(self.deadline, self.lease.client().query(sql, params))
            .await
            .map_err(|_| Error::Deadline)?
            .map_err(|error| {
                self.note(&error);
                Error::Unavailable
            })
    }
    /// As `query`, but a database refusal keeps its SQLSTATE so that the
    /// storage module can classify it (ADR-007 D7.9). Nothing leaves storage.
    pub async fn query_classified(
        &self,
        sql: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<std::result::Result<Vec<Row>, SqlState>> {
        self.counted();
        match timeout_at(self.deadline, self.lease.client().query(sql, params))
            .await
            .map_err(|_| Error::Deadline)?
        {
            Ok(rows) => Ok(Ok(rows)),
            Err(error) => {
                self.note(&error);
                error.code().cloned().map(Err).ok_or(Error::Unavailable)
            }
        }
    }
    pub async fn precheck(&self, action: Action, resource: Resource<'a>) -> Result<Precheck<'a>> {
        let request = self.member()?;
        require(self.lease.role() != Role::Worker)?;
        self.current_member(request).await?;
        self.entitlement(request).await?;
        let precheck = Precheck { action, resource };
        self.permissions(request, &precheck).await?;
        Ok(precheck)
    }
    /// An active organization, an active membership at the request's epoch.
    async fn current_member(&self, request: &TrustedRequest) -> Result<()> {
        let rows = self.query("SELECT o.status,m.status,m.membership_epoch FROM genui.organizations o JOIN genui.memberships m USING(organization_id) WHERE o.organization_id=$1::text::uuid AND m.principal_id=$2::text::uuid", &[&request.organization,&request.principal]).await?;
        let row = rows.first().ok_or(Error::Denied)?;
        require(
            text(row, 0)? == "active"
                && text(row, 1)? == "active"
                && integer(row, 2)? == request.epoch,
        )
    }
    /// APP-076 G9 (the workspace listing): the member half of `precheck` and
    /// `fence` with no resource permission of its own: current membership,
    /// epoch and entitlement, the `lock_member_authority` fence, and the
    /// entitlement again after it. The permit names the organization with
    /// `Discover`, which `publication` and `record` refuse, so it authorizes
    /// nothing by itself: the listing judges every item with `Permit::also`
    /// (the same permission clauses the routes' own prechecks apply).
    pub(super) async fn fence_member<'t>(&'t mut self) -> Result<Permit<'t, 'a>> {
        let request = self.member()?;
        require(self.lease.role() != Role::Worker)?;
        self.current_member(request).await?;
        self.entitlement(request).await?;
        self.lock_member(request).await?;
        self.entitlement(request).await?;
        Ok(Permit {
            transaction: self,
            request,
            precheck: Precheck {
                action: Action::Discover,
                resource: Resource::Organization,
            },
        })
    }
    /// The batch form of `precheck` and `fence` for `record_read` on many
    /// saved works at once (the canvas page: the work page's and the record
    /// page's authorization, per work, in one transaction): current
    /// membership, epoch and entitlement, every work's [`Grants`] (the
    /// `permissions` clauses), the `lock_member_authority` fence, the
    /// entitlement again and every work's grants re-read after it. A member
    /// refusal is `Denied` for the whole batch, as it is for each work's own
    /// precheck. Per work, `record_read` holds only when it held before and
    /// after the fence (`precheck` then `fence`); `discover` and
    /// `record_write` are the after-fence re-reads (`Permit::also`). The
    /// permit names the organization with `Discover`, like `fence_member`'s,
    /// so it authorizes nothing by itself.
    pub(super) async fn fence_works<'t>(
        &'t mut self,
        works: &[(&str, &str)],
    ) -> Result<(Permit<'t, 'a>, Vec<Grants>)> {
        let request = self.member()?;
        require(self.lease.role() != Role::Worker)?;
        self.current_member(request).await?;
        self.entitlement(request).await?;
        let before = self.grants(request, works).await?;
        self.lock_member(request).await?;
        self.entitlement(request).await?;
        let after = self.grants(request, works).await?;
        let grants = before
            .iter()
            .zip(after)
            .map(|(before, after)| Grants {
                record_read: before.record_read && after.record_read,
                ..after
            })
            .collect();
        Ok((
            Permit {
                transaction: self,
                request,
                precheck: Precheck {
                    action: Action::Discover,
                    resource: Resource::Organization,
                },
            },
            grants,
        ))
    }
    /// APP-122: the member half of `fence_works` inside a READ ONLY snapshot
    /// (`begin_read_only`): current membership, epoch and entitlement, and
    /// every work's [`Grants`], with no `lock_member_authority` fence (a
    /// read-only transaction takes no row lock). The checks and the reads
    /// that follow share one snapshot, so a revocation committed after it
    /// began is not seen by either; nothing here can write.
    /// APP-124 (Ask on a folder): the member half of a `READ ONLY` read with
    /// no work in it: current membership, epoch and entitlement, without the
    /// `lock_member_authority` row lock (refused in a read-only snapshot);
    /// the snapshot itself orders the checks with every read after them.
    pub(super) async fn read_only_member(&self) -> Result<()> {
        let request = self.member()?;
        require(self.lease.role() != Role::Worker)?;
        self.current_member(request).await?;
        self.entitlement(request).await
    }
    pub(super) async fn read_only_grants(&self, works: &[(&str, &str)]) -> Result<Vec<Grants>> {
        let request = self.member()?;
        require(self.lease.role() != Role::Worker)?;
        self.current_member(request).await?;
        self.entitlement(request).await?;
        self.grants(request, works).await
    }
    /// For each `(app, work)`, in one statement, exactly the `permissions`
    /// clauses for `discover` on the app and `record_read`/`record_write` on
    /// the work (an identifier that is not a canonical UUID is refused, as
    /// `permissions` refuses it).
    async fn grants(
        &self,
        request: &TrustedRequest,
        works: &[(&str, &str)],
    ) -> Result<Vec<Grants>> {
        let mut grants = vec![Grants::default(); works.len()];
        let sent: Vec<usize> = (0..works.len())
            .filter(|&i| valid_id(works[i].0) && valid_id(works[i].1))
            .collect();
        if sent.is_empty() {
            return Ok(grants);
        }
        let apps: Vec<&str> = sent.iter().map(|&i| works[i].0).collect();
        let ids: Vec<&str> = sent.iter().map(|&i| works[i].1).collect();
        let principal = "$2::text::uuid";
        let sql = format!(
            "SELECT c.n::bigint,EXISTS(SELECT 1 FROM genui.saved_work w WHERE w.organization_id=$1::text::uuid AND w.app_id=c.app::uuid AND w.work_id=c.work::uuid AND {}),EXISTS(SELECT 1 FROM genui.saved_work w WHERE w.organization_id=$1::text::uuid AND w.app_id=c.app::uuid AND w.work_id=c.work::uuid AND {}),EXISTS(SELECT 1 FROM genui.saved_work w WHERE w.organization_id=$1::text::uuid AND w.app_id=c.app::uuid AND w.work_id=c.work::uuid AND {}),EXISTS(SELECT 1 FROM genui.saved_work w WHERE w.organization_id=$1::text::uuid AND w.app_id=c.app::uuid AND w.work_id=c.work::uuid AND {} AND {}) FROM unnest($3::text[],$4::text[]) WITH ORDINALITY AS c(app,work,n)",
            discover_clause(principal),
            work_clause(principal, "record_read"),
            work_clause(principal, "record_write"),
            work_clause(principal, "record_read"),
            reviewer_clause("w.app_id", "w.work_id", principal),
        );
        let rows = self
            .query(
                &sql,
                &[&request.organization, &request.principal, &apps, &ids],
            )
            .await?;
        for row in &rows {
            let n = usize::try_from(integer(row, 0)?).map_err(|_| Error::Unavailable)?;
            let index = *n
                .checked_sub(1)
                .and_then(|n| sent.get(n))
                .ok_or(Error::Unavailable)?;
            grants[index] = Grants {
                discover: boolean(row, 1)?,
                record_read: boolean(row, 2)?,
                record_write: boolean(row, 3)?,
                record_review: boolean(row, 4)?,
            };
        }
        Ok(grants)
    }
    async fn lock_member(&self, request: &TrustedRequest) -> Result<()> {
        let rows = self.query("SELECT organization_active,member_active,membership_epoch FROM genui_private.lock_member_authority($1::text::uuid,$2::text::uuid,$3)", &[&request.organization,&request.principal,&request.epoch]).await?;
        let row = rows.first().ok_or(Error::Denied)?;
        require(boolean(row, 0)? && boolean(row, 1)? && integer(row, 2)? == request.epoch)
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
            // APP-192 (D9): `discover` and `run` on an app are also held by
            // every active member while a live organization-wide share of a
            // work of that app exists (the app-level rows a personal share
            // copies); `edit_definition` and `review_publish` are grants only.
            Resource::App(app) if matches!(precheck.action, Action::Discover | Action::Run) => {
                require(valid_id(app))?;
                self.query(&format!("SELECT 1 FROM genui.apps a WHERE a.organization_id=$1::text::uuid AND a.app_id=$3::text::uuid AND (EXISTS(SELECT 1 FROM genui.app_permissions p WHERE p.organization_id=a.organization_id AND p.principal_id=$2::text::uuid AND p.app_id=a.app_id AND p.permission=$4) OR {})", organization_share("a.app_id", None, "$2::text::uuid", None)), &[org,principal,&app,&action]).await?
            }
            Resource::App(app) if matches!(precheck.action, Action::EditDefinition | Action::ReviewPublish) => {
                require(valid_id(app))?;
                self.query("SELECT 1 FROM genui.app_permissions p JOIN genui.apps a USING(organization_id,app_id) WHERE p.organization_id=$1::text::uuid AND p.principal_id=$2::text::uuid AND p.app_id=$3::text::uuid AND p.permission=$4", &[org,principal,&app,&action]).await?
            }
            Resource::Source(source) if precheck.action == Action::SourceRead => {
                require(valid_id(source))?;
                self.query("SELECT 1 FROM genui.source_permissions p JOIN genui.sources s USING(organization_id,source_id) WHERE p.organization_id=$1::text::uuid AND p.principal_id=$2::text::uuid AND p.source_id=$3::text::uuid AND p.permission='source_read'", &[org,principal,&source]).await?
            }
            // ADR-007 amendment 2026-10-29 (APP-010 review M2): the work's
            // creator, until the operator revokes it, holds record_read and
            // record_write on that work. Creator authority replaces only the
            // work_permissions lookup; the app `run` clause and every
            // dependency's live `source_read` still apply, and membership
            // revocation ends it at the precheck above.
            // APP-191/APP-192: `record_read` and `record_write` are the
            // `work_clause` (a grant, the unrevoked creator or a live
            // organization-wide share of the work); `record_review` is
            // `record_read` and the reviewer rule.
            Resource::Work { app, work } if matches!(precheck.action, Action::RecordRead | Action::RecordWrite) => {
                require(valid_id(app) && valid_id(work))?;
                let permission = if precheck.action == Action::RecordRead { "record_read" } else { "record_write" };
                self.query(&format!("SELECT 1 FROM genui.saved_work w WHERE w.organization_id=$1::text::uuid AND w.app_id=$3::text::uuid AND w.work_id=$4::text::uuid AND {}", work_clause("$2::text::uuid", permission)), &[org,principal,&app,&work]).await?
            }
            Resource::Work { app, work } if precheck.action == Action::RecordReview => {
                require(valid_id(app) && valid_id(work))?;
                self.query(&format!("SELECT 1 FROM genui.saved_work w WHERE w.organization_id=$1::text::uuid AND w.app_id=$3::text::uuid AND w.work_id=$4::text::uuid AND {} AND {}", work_clause("$2::text::uuid", "record_read"), reviewer_clause("w.app_id", "w.work_id", "$2::text::uuid")), &[org,principal,&app,&work]).await?
            }
            Resource::Work { app, work } if matches!(precheck.action, Action::ShareWork | Action::Export) => {
                require(valid_id(app) && valid_id(work))?;
                self.query("SELECT 1 FROM genui.saved_work w WHERE w.organization_id=$1::text::uuid AND w.app_id=$3::text::uuid AND w.work_id=$4::text::uuid AND (EXISTS(SELECT 1 FROM genui.work_permissions p WHERE p.organization_id=w.organization_id AND p.principal_id=$2::text::uuid AND p.app_id=w.app_id AND p.work_id=w.work_id AND p.permission=$5) OR ($5 IN ('record_read','record_write') AND w.created_by=$2::text::uuid AND w.creator_revoked_at IS NULL)) AND EXISTS(SELECT 1 FROM genui.app_permissions a WHERE a.organization_id=w.organization_id AND a.principal_id=$2::text::uuid AND a.app_id=w.app_id AND a.permission='run') AND NOT EXISTS(SELECT 1 FROM genui.work_source_dependencies d JOIN genui.source_revisions r ON r.organization_id=d.organization_id AND r.source_id=d.source_id AND r.revision_id=d.source_revision_id AND r.normalized_digest=d.source_digest WHERE d.organization_id=w.organization_id AND d.app_id=w.app_id AND d.work_id=w.work_id AND (r.deleted_at IS NOT NULL OR NOT EXISTS(SELECT 1 FROM genui.source_permissions s WHERE s.organization_id=d.organization_id AND s.principal_id=$2::text::uuid AND s.source_id=d.source_id AND s.permission='source_read')))", &[org,principal,&app,&work,&action]).await?
            }
            _ => return Err(Error::Denied),
        };
        require(!rows.is_empty())
    }
    pub async fn fence<'t>(&'t mut self, precheck: Precheck<'a>) -> Result<Permit<'t, 'a>> {
        let request = self.member()?;
        self.lock_member(request).await?;
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
    /// D7.6: `Ok(true)` committed; `Ok(false)` PostgreSQL refused the COMMIT
    /// with 40001 and rolled the transaction back, so a retry is safe. Any other
    /// COMMIT failure stays `Error::UnknownOutcome` and is never retried.
    pub async fn commit_serializable(self) -> Result<bool> {
        self.lease.commit_serializable(self.deadline).await
    }
}

/// APP-113 review (storage 4, 7): the `record_read` clause of `permissions`
/// as an SQL predicate over a `genui.saved_work` row aliased `w` and a
/// principal given as an SQL expression (a placeholder or a column): a work
/// grant or the unrevoked creator's authority, `run` on the app, and live
/// `source_read` on every dependency. Listings filter candidates with it
/// before their `LIMIT`, and a box file grant counts only while it holds for
/// the grant's work; the authoritative judgment stays `precheck`/`also`.
pub(super) fn record_read_clause(principal: &str) -> String {
    work_clause(principal, "record_read")
}

/// The `permissions` clause for `record_read` or `record_write` (the two
/// permissions the unrevoked creator also holds) on a `genui.saved_work`
/// row `w`, for `principal` (an SQL expression); `permission` is one of
/// those two literals, never request data. APP-192 (D9): a live
/// organization-wide share of the work ([`organization_share`]) stands in
/// for the work grant and for `run` on the app, for every active member.
pub(super) fn work_clause(principal: &str, permission: &'static str) -> String {
    debug_assert!(matches!(permission, "record_read" | "record_write"));
    let shared = organization_share("w.app_id", Some("w.work_id"), principal, Some(permission));
    let run = organization_share("w.app_id", Some("w.work_id"), principal, None);
    format!("(EXISTS(SELECT 1 FROM genui.work_permissions p WHERE p.organization_id=w.organization_id AND p.principal_id={principal} AND p.app_id=w.app_id AND p.work_id=w.work_id AND p.permission='{permission}') OR (w.created_by={principal} AND w.creator_revoked_at IS NULL) OR {shared}) AND (EXISTS(SELECT 1 FROM genui.app_permissions a WHERE a.organization_id=w.organization_id AND a.principal_id={principal} AND a.app_id=w.app_id AND a.permission='run') OR {run}) AND NOT EXISTS(SELECT 1 FROM genui.work_source_dependencies d JOIN genui.source_revisions r ON r.organization_id=d.organization_id AND r.source_id=d.source_id AND r.revision_id=d.source_revision_id AND r.normalized_digest=d.source_digest WHERE d.organization_id=w.organization_id AND d.app_id=w.app_id AND d.work_id=w.work_id AND (r.deleted_at IS NOT NULL OR NOT EXISTS(SELECT 1 FROM genui.source_permissions s WHERE s.organization_id=d.organization_id AND s.principal_id={principal} AND s.source_id=d.source_id AND s.permission='source_read')))")
}

/// APP-192 (D9): a live organization-wide share (`genui.work_shares` with no
/// grantee and no `revoked_at`) of a work of app `{app}` (and of work
/// `{work}` when given), while `{principal}` is an active member of the
/// scoped organization; `permission`, when given, narrows the share's role to
/// those that carry it (viewer: `record_read`; contributor: also
/// `record_write`; reviewer: also `record_review`). Arguments are SQL
/// expressions. Aliases are private to this clause (`os`, `om`).
pub(super) fn organization_share(
    app: &str,
    work: Option<&str>,
    principal: &str,
    permission: Option<&'static str>,
) -> String {
    let roles = match permission {
        None | Some("record_read") => "'viewer','contributor','reviewer'",
        Some("record_write") => "'contributor','reviewer'",
        Some("record_review") => "'reviewer'",
        Some(other) => unreachable!("not a role-carried permission: {other}"),
    };
    let work = work
        .map(|work| format!(" AND os.work_id={work}"))
        .unwrap_or_default();
    format!("EXISTS(SELECT 1 FROM genui.work_shares os WHERE os.organization_id={SCOPED_ORG} AND os.app_id={app}{work} AND os.grantee_id IS NULL AND os.revoked_at IS NULL AND os.role IN ({roles}) AND EXISTS(SELECT 1 FROM genui.memberships om WHERE om.organization_id=os.organization_id AND om.principal_id={principal} AND om.status='active'))")
}

/// The listing's `discover` on the app of a `genui.saved_work` row `w` for
/// `principal`: a `discover` grant, or (APP-192) a live organization-wide
/// share of the work. The one clause behind `grants`, `listable` and
/// `openable_principals`.
pub(super) fn discover_clause(principal: &str) -> String {
    format!(
        "(EXISTS(SELECT 1 FROM genui.app_permissions x WHERE x.organization_id=w.organization_id AND x.principal_id={principal} AND x.app_id=w.app_id AND x.permission='discover') OR {})",
        organization_share("w.app_id", Some("w.work_id"), principal, None)
    )
}

/// `discover` on a `genui.apps` row `{app_alias}` for `principal`: a grant,
/// or a live organization-wide share of any work of the app (labels).
pub(super) fn app_discover_clause(app_alias: &str, principal: &str) -> String {
    format!(
        "(EXISTS(SELECT 1 FROM genui.app_permissions p WHERE p.organization_id={app_alias}.organization_id AND p.principal_id={principal} AND p.app_id={app_alias}.app_id AND p.permission='discover') OR {})",
        organization_share(&format!("{app_alias}.app_id"), None, principal, None)
    )
}

/// The transaction's scoped organization as an SQL expression.
const SCOPED_ORG: &str = "nullif(current_setting('genui.organization_id',true),'')::uuid";
/// The transaction's scoped principal as an SQL expression: set for every
/// member transaction (`Transaction::open`), NULL for a guest or nonmember
/// entry, so that a guest is never a creator and never a reviewer.
pub(super) const SCOPED: &str = "nullif(current_setting('genui.principal_id',true),'')::uuid";

/// The `private-entries` campaign's bypass (APP-191 acceptance: every check
/// fails with the enforcement bypassed): `own_rows` becomes TRUE, `hidden`
/// empty, `own_collections` zero and `compose` skips the `reviewer_only` gate
/// while set. Test builds only; a campaign runs alone, so no other test
/// reads it.
#[cfg(test)]
pub(crate) static BYPASS: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
pub(super) fn bypassed() -> bool {
    BYPASS.load(Ordering::SeqCst)
}
#[cfg(not(test))]
#[inline]
pub(super) fn bypassed() -> bool {
    false
}

/// APP-191 (D7): the one row rule of an `own` collection, over a
/// `genui.internal_record_revisions` row aliased `{row}` of work `{work}` of
/// app `{app}`: the row is the principal's own (`creator_id`), or its
/// collection is not hidden from the principal ([`hidden`]). Every record
/// read and write of storage carries it (the scan test
/// `every_record_statement_carries_own_rows` enforces that); `{principal}` is
/// [`SCOPED`] in a member or guest transaction, or a membership column in a
/// per-member judgment. Arguments are SQL expressions, never request data.
pub(super) fn own_rows(row: &str, app: &str, work: &str, principal: &str) -> String {
    if bypassed() {
        return "TRUE".to_owned();
    }
    format!(
        "(coalesce({row}.creator_id={principal},false) OR {row}.collection_id<>ALL({}))",
        hidden(app, work, principal)
    )
}

/// The collections of work `{work}` of app `{app}` hidden from `{principal}`:
/// the pinned publication's record schemas declared `visibility: "own"`
/// (`definition_record_schemas.owner_only`, derived in the database from the
/// stored canonical schema, migration 0024) unless the principal is a
/// reviewer of the work ([`reviewer_clause`]). An `ARRAY(...)` subquery with
/// no outer row reference, so PostgreSQL evaluates it once per statement (an
/// InitPlan) and each row costs one array comparison. Aliases are private
/// (`hc`, `hp`, `hd`).
pub(super) fn hidden(app: &str, work: &str, principal: &str) -> String {
    if bypassed() {
        return "ARRAY[]::uuid[]".to_owned();
    }
    format!(
        "ARRAY(SELECT hc.collection_id FROM genui.record_collections hc JOIN genui.publication_record_schemas hp ON hp.organization_id=hc.organization_id AND hp.app_id=hc.app_id AND hp.publication_id=hc.publication_id AND hp.schema_id=hc.schema_id AND hp.schema_version=hc.schema_version JOIN genui.definition_record_schemas hd ON hd.organization_id=hp.organization_id AND hd.app_id=hp.app_id AND hd.revision_id=hp.definition_revision_id AND hd.schema_id=hp.schema_id AND hd.schema_version=hp.schema_version AND hd.digest=hp.digest WHERE hc.organization_id={SCOPED_ORG} AND hc.app_id={app} AND hc.work_id={work} AND hd.owner_only AND NOT {})",
        reviewer_clause(app, work, principal)
    )
}

/// APP-191: whether `{principal}` is a reviewer of work `{work}` of app
/// `{app}`: an active member of the scoped organization who is the app's
/// owner, the work's unrevoked creator (Victor, 2026-11-14), a holder of
/// `record_review` on the work, or covered by a live organization-wide share
/// with the reviewer role. Arguments are SQL expressions. Aliases are private
/// (`rm`, `ra`, `rw`, `rp`, `rs`), so the clause composes inside any statement.
pub(super) fn reviewer_clause(app: &str, work: &str, principal: &str) -> String {
    format!("(EXISTS(SELECT 1 FROM genui.memberships rm WHERE rm.organization_id={SCOPED_ORG} AND rm.principal_id={principal} AND rm.status='active') AND (EXISTS(SELECT 1 FROM genui.apps ra WHERE ra.organization_id={SCOPED_ORG} AND ra.app_id={app} AND ra.owner_principal_id={principal}) OR EXISTS(SELECT 1 FROM genui.saved_work rw WHERE rw.organization_id={SCOPED_ORG} AND rw.app_id={app} AND rw.work_id={work} AND rw.created_by={principal} AND rw.creator_revoked_at IS NULL) OR EXISTS(SELECT 1 FROM genui.work_permissions rp WHERE rp.organization_id={SCOPED_ORG} AND rp.principal_id={principal} AND rp.app_id={app} AND rp.work_id={work} AND rp.permission='record_review') OR EXISTS(SELECT 1 FROM genui.work_shares rs WHERE rs.organization_id={SCOPED_ORG} AND rs.app_id={app} AND rs.work_id={work} AND rs.grantee_id IS NULL AND rs.revoked_at IS NULL AND rs.role='reviewer')))")
}

/// How many collections of work `work` of app `app` are declared `own`
/// (APP-191): a live (guest) share of such a work is refused, since a guest
/// is nobody's creator and no reviewer.
pub(super) async fn own_collections(
    transaction: &Transaction<'_>,
    organization: &str,
    app: &str,
    work: &str,
) -> Result<i64> {
    if bypassed() {
        return Ok(0);
    }
    let rows = transaction.query("SELECT count(*)::bigint FROM genui.record_collections hc JOIN genui.publication_record_schemas hp ON hp.organization_id=hc.organization_id AND hp.app_id=hc.app_id AND hp.publication_id=hc.publication_id AND hp.schema_id=hc.schema_id AND hp.schema_version=hc.schema_version JOIN genui.definition_record_schemas hd ON hd.organization_id=hp.organization_id AND hd.app_id=hp.app_id AND hd.revision_id=hp.definition_revision_id AND hd.schema_id=hp.schema_id AND hd.schema_version=hp.schema_version AND hd.digest=hp.digest WHERE hc.organization_id=$1::text::uuid AND hc.app_id=$2::text::uuid AND hc.work_id=$3::text::uuid AND hd.owner_only", &[&organization,&app,&work]).await?;
    integer(rows.first().ok_or(Error::Unavailable)?, 0)
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

/// One work's grants in a batch (`Transaction::grants`).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Grants {
    pub discover: bool,
    pub record_read: bool,
    pub record_write: bool,
    /// APP-191: `Action::RecordReview` (the reviewer rule, after the fence).
    pub record_review: bool,
}

impl<'r> Permit<'_, 'r> {
    /// The fenced transaction, for the storage operation that obtained this
    /// permit; nothing outside `storage` ever sees it.
    pub(super) fn transaction(&self) -> &Transaction<'r> {
        self.transaction
    }
    /// Rechecks the wall-clock entitlement after an additional wait while
    /// this permit still holds the membership/epoch fence.
    pub(super) async fn recheck_entitlement(&self) -> Result<()> {
        self.transaction.entitlement(self.request).await
    }
    /// A further permission of the same member, re-read after the fence (for
    /// example `source_read` on each source a saved work binds).
    pub(super) async fn also(&self, action: Action, resource: Resource<'_>) -> Result<()> {
        self.transaction
            .permissions(self.request, &Precheck { action, resource })
            .await
    }
    /// [`Permit::also`] for `discover`, `record_read` and `record_write` on
    /// many works in one statement (the same clauses, re-read after the
    /// fence).
    pub(super) async fn grants(&self, works: &[(&str, &str)]) -> Result<Vec<Grants>> {
        self.transaction.grants(self.request, works).await
    }
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
        // APP-191: a record of an `own` collection another member created is
        // absent to a non-reviewer (the uniform 404), like an unknown record.
        let rows = self.transaction.query(&format!("SELECT r.record_id::text,r.current_version FROM genui.internal_records r JOIN genui.internal_record_revisions v ON v.organization_id=r.organization_id AND v.app_id=r.app_id AND v.work_id=r.work_id AND v.collection_id=r.collection_id AND v.record_id=r.record_id AND v.version=r.current_version WHERE r.organization_id=$1::text::uuid AND r.app_id=$2::text::uuid AND r.work_id=$3::text::uuid AND r.collection_id=$4::text::uuid AND r.record_id=$5::text::uuid AND {}", own_rows("v", "$2::text::uuid", "$3::text::uuid", SCOPED)), &[&self.request.organization,&app,&work,&collection,&record]).await?;
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
