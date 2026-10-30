use super::{catalog::Catalog, registry::Registry, Error, RegistryIdentity, Result};
use crate::config::{now, Config};
use deadpool::managed::{Manager, Metrics, Object, Pool, RecycleError, RecycleResult};
use rustls_pki_types::{pem::PemObject, CertificateDer};
use std::{
    future::Future,
    io::Read,
    path::PathBuf,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, OnceLock,
    },
    task::{Context, Poll},
    time::Duration,
};
use tokio::{
    sync::{Notify, OwnedSemaphorePermit, Semaphore},
    task::JoinHandle,
    time::{timeout_at, Instant},
};
use tokio_postgres::{config::SslMode, Client};
use tokio_postgres_rustls::MakeRustlsConnect;

pub(super) const ACQUIRE: Duration = Duration::from_secs(5);
pub(super) const TRANSACTION: Duration = Duration::from_secs(3);
pub(super) const CLEANUP: Duration = Duration::from_secs(2);
const EXPIRY_MARGIN: u64 = 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Role {
    Workspace,
    Control,
    Data,
    Worker,
}
impl Role {
    pub fn name(self) -> &'static str {
        match self {
            Self::Workspace => "genui_workspace",
            Self::Control => "genui_control",
            Self::Data => "genui_data",
            Self::Worker => "genui_worker",
        }
    }
    fn index(self) -> usize {
        match self {
            Self::Workspace => 0,
            Self::Control => 1,
            Self::Data => 2,
            Self::Worker => 3,
        }
    }
}

/// Deployment-owned paths/endpoint. Request data cannot configure a connection.
pub struct Settings {
    pub(super) credentials: PathBuf,
    pub(super) registry: PathBuf,
    ca: PathBuf,
    host: String,
    port: u16,
    #[cfg(test)]
    pub(crate) hooks: Arc<TestHooks>,
}
impl Settings {
    pub fn new(
        credentials: PathBuf,
        registry: PathBuf,
        ca: PathBuf,
        rds_hostname: String,
    ) -> Result<Self> {
        if !rds_hostname.ends_with(".us-east-1.rds.amazonaws.com")
            || rds_hostname.len() > 253
            || !rds_hostname
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'-'))
        {
            return Err(Error::Configuration);
        }
        Ok(Self {
            credentials,
            registry,
            ca,
            host: rds_hostname,
            port: 5432,
            #[cfg(test)]
            hooks: Arc::new(TestHooks::default()),
        })
    }
    #[cfg(test)]
    pub(super) fn fixture(
        credentials: PathBuf,
        registry: PathBuf,
        ca: PathBuf,
        host: &str,
        port: u16,
    ) -> Self {
        Self {
            credentials,
            registry,
            ca,
            host: host.into(),
            port,
            hooks: Arc::new(TestHooks::default()),
        }
    }
}

// Compiled only in the isolated test executable. No feature, environment or
// production constructor can select these lifecycle barriers.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct TestHooks {
    phase: std::sync::Mutex<Option<&'static str>>,
    pub reached: Notify,
    pub release: Notify,
}
#[cfg(test)]
impl TestHooks {
    pub fn arm(&self, phase: &'static str) {
        *self.phase.lock().unwrap() = Some(phase);
    }
    async fn checkpoint(&self, phase: &'static str) {
        let stop = {
            let mut armed = self.phase.lock().unwrap();
            if *armed == Some(phase) {
                armed.take();
                true
            } else {
                false
            }
        };
        if stop {
            self.reached.notify_one();
            self.release.notified().await;
        }
    }
}

struct Capacity {
    all: Arc<Semaphore>,
    roles: [Arc<Semaphore>; 4],
    admitted: [Arc<Semaphore>; 4],
}
fn capacity() -> &'static Capacity {
    static VALUE: OnceLock<Capacity> = OnceLock::new();
    VALUE.get_or_init(|| Capacity {
        all: Arc::new(Semaphore::new(8)),
        roles: std::array::from_fn(|_| Arc::new(Semaphore::new(2))),
        admitted: std::array::from_fn(|_| Arc::new(Semaphore::new(8))),
    })
}

struct DriverPermits {
    _role: OwnedSemaphorePermit,
    _all: OwnedSemaphorePermit,
}
struct Closed {
    value: AtomicBool,
    notify: Notify,
}
struct ClosedNotice(Arc<Closed>);
impl Drop for ClosedNotice {
    fn drop(&mut self) {
        self.0.value.store(true, Ordering::Release);
        self.0.notify.notify_waiters();
    }
}
/// Declaration-order destruction: transport first, capacity second, notice last.
/// Abort requests cannot release slots before the owned transport is dropped.
struct DriverFuture<F> {
    connection: F,
    _permits: DriverPermits,
    _closed: ClosedNotice,
}
impl<F: Future<Output = std::result::Result<(), tokio_postgres::Error>> + Unpin> Future
    for DriverFuture<F>
{
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        Pin::new(&mut self.get_mut().connection)
            .poll(cx)
            .map(|_| ())
    }
}
pub(super) struct Driver {
    handle: JoinHandle<()>,
    _closed: Arc<Closed>,
}
impl Drop for Driver {
    fn drop(&mut self) {
        self.handle.abort();
    }
}
impl Driver {
    fn start<F>(connection: F, permits: DriverPermits) -> Self
    where
        F: Future<Output = std::result::Result<(), tokio_postgres::Error>> + Unpin + Send + 'static,
    {
        let closed = Arc::new(Closed {
            value: AtomicBool::new(false),
            notify: Notify::new(),
        });
        let handle = tokio::spawn(DriverFuture {
            connection,
            _permits: permits,
            _closed: ClosedNotice(closed.clone()),
        });
        Self {
            handle,
            _closed: closed,
        }
    }
}

pub(super) struct Connection {
    pub client: Client,
    _driver: Driver,
    pub generation: String,
    pub fingerprint: String,
    pub expires_at: u64,
    pub clean: bool,
}

#[derive(Clone)]
pub(super) struct RoleManager {
    settings: Arc<Settings>,
    tls: MakeRustlsConnect,
    catalog: Arc<Catalog>,
    pub role: Role,
}
impl RoleManager {
    fn configuration(&self) -> Result<Config> {
        let value = Config::load(&self.settings.credentials).map_err(|_| Error::Configuration)?;
        if value.database_host != self.settings.host
            || value.database_name != "genui"
            || value.expires_at <= now().saturating_add(EXPIRY_MARGIN + TRANSACTION.as_secs())
        {
            return Err(Error::Configuration);
        }
        Ok(value)
    }
    async fn connect(&self, value: &Config) -> Result<Connection> {
        let cap = capacity();
        // These are acquired before opening a transport. During cancellation of
        // connect(), the connect future/transport drops before these locals.
        let permits = DriverPermits {
            _role: cap.roles[self.role.index()]
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| Error::Unavailable)?,
            _all: cap
                .all
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| Error::Unavailable)?,
        };
        let credential = match self.role {
            Role::Workspace => &value.workspace,
            Role::Control => &value.control,
            Role::Data => &value.data,
            Role::Worker => &value.worker,
        };
        if credential.username != self.role.name() {
            return Err(Error::Configuration);
        }
        let mut config = tokio_postgres::Config::new();
        config
            .host(&self.settings.host)
            .port(self.settings.port)
            .dbname("genui")
            .user(self.role.name())
            .password(credential.secret.expose_to_database_adapter())
            .ssl_mode(SslMode::Require)
            .connect_timeout(ACQUIRE)
            // Startup parameters precede every SQL statement, including driver
            // type discovery. A database/role default must not select objects.
            .options(r"-c search_path=pg_catalog,\ pg_temp")
            .application_name("genui-native-storage");
        let (client, connection) = config
            .connect(self.tls.clone())
            .await
            .map_err(|_| Error::Unavailable)?;
        let driver = Driver::start(connection, permits);
        let mut connected = Connection {
            client,
            _driver: driver,
            generation: value.generation.clone(),
            fingerprint: value.file_sha256.clone(),
            expires_at: value.expires_at,
            clean: false,
        };
        #[cfg(test)]
        self.settings.hooks.checkpoint("connect").await;
        self.verify_admission(&connected.client).await?;
        reset(&mut connected, self.role).await?;
        Ok(connected)
    }
    async fn verify_admission(&self, client: &Client) -> Result<()> {
        // Only built-in catalog objects may be touched before the structural
        // comparison. Even the version relation could have become a view.
        verify_identity(client, self.role).await?;
        self.catalog.verify(client).await?;
        verify_schema_version(client).await
    }
}
impl Manager for RoleManager {
    type Type = Connection;
    type Error = Error;
    async fn create(&self) -> Result<Connection> {
        for attempt in 0..2 {
            let value = self.configuration()?;
            let connected = self.connect(&value).await;
            let current = self.configuration()?;
            if current.generation == value.generation && current.file_sha256 == value.file_sha256 {
                return connected;
            }
            drop(connected);
            if attempt == 1 {
                return Err(Error::Configuration);
            }
        }
        Err(Error::Unavailable)
    }
    async fn recycle(&self, connection: &mut Connection, _: &Metrics) -> RecycleResult<Error> {
        if !connection.clean || connection.client.is_closed() {
            return Err(RecycleError::Backend(Error::Unavailable));
        }
        connection.clean = false;
        let current = self.configuration().map_err(RecycleError::Backend)?;
        if current.generation != connection.generation
            || current.file_sha256 != connection.fingerprint
        {
            return Err(RecycleError::Backend(Error::Configuration));
        }
        verify_baseline(&connection.client, self.role)
            .await
            .map_err(RecycleError::Backend)?;
        self.verify_admission(&connection.client)
            .await
            .map_err(RecycleError::Backend)?;
        #[cfg(test)]
        self.settings.hooks.checkpoint("recycle").await;
        connection.clean = true;
        Ok(())
    }
}

pub(super) struct RolePool {
    pub pool: Pool<RoleManager>,
    manager: Arc<RoleManager>,
    pub registry: Arc<Registry>,
}
impl RolePool {
    pub fn role(&self) -> Role {
        self.manager.role
    }
    async fn acquire(self: &Arc<Self>) -> Result<Lease> {
        let deadline = Instant::now() + ACQUIRE;
        let admitted = capacity().admitted[self.manager.role.index()]
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::Unavailable)?;
        self.manager.configuration()?;
        let mut object = timeout_at(deadline, self.pool.get())
            .await
            .map_err(|_| Error::Deadline)?
            .map_err(|_| Error::Unavailable)?;
        // No await between ownership acquisition and the non-reusable guard.
        object.clean = false;
        let lease = Lease {
            object: Some(object),
            owner: self.clone(),
            _admitted: admitted,
        };
        let current = self.manager.configuration()?;
        if current.generation != lease.connection().generation
            || current.file_sha256 != lease.connection().fingerprint
        {
            return Err(Error::Configuration);
        }
        Ok(lease)
    }
}

pub(super) struct Lease {
    object: Option<Object<RoleManager>>,
    owner: Arc<RolePool>,
    _admitted: OwnedSemaphorePermit,
}
impl Drop for Lease {
    fn drop(&mut self) {
        if let Some(object) = self.object.take() {
            // No asynchronous cleanup is delegated to ordinary pool return.
            drop(Object::take(object));
        }
    }
}
impl Lease {
    #[cfg(test)]
    pub async fn checkpoint(&self, phase: &'static str) {
        self.owner.manager.settings.hooks.checkpoint(phase).await;
    }
    pub fn connection(&self) -> &Connection {
        self.object.as_ref().expect("owned lease")
    }
    pub fn client(&self) -> &Client {
        &self.connection().client
    }
    pub fn role(&self) -> Role {
        self.owner.manager.role
    }
    pub fn transaction_deadline(&self) -> Result<Instant> {
        let seconds = self
            .connection()
            .expires_at
            .saturating_sub(now().saturating_add(EXPIRY_MARGIN));
        if seconds < TRANSACTION.as_secs() {
            return Err(Error::Configuration);
        }
        Ok(Instant::now() + TRANSACTION)
    }
    pub async fn acquire(owner: &Arc<RolePool>) -> Result<Self> {
        owner.acquire().await
    }
    pub async fn finish(self, commit: bool, transaction_deadline: Instant) -> Result<()> {
        let command = if commit { "COMMIT" } else { "ROLLBACK" };
        // APP-013: the command campaign pauses here to end the backend before
        // COMMIT, so the answer is lost and the outcome unknown (D7.9).
        #[cfg(test)]
        if commit {
            self.checkpoint("commit").await;
        }
        match timeout_at(transaction_deadline, self.client().batch_execute(command)).await {
            Ok(Ok(())) => (),
            _ if commit => return Err(Error::UnknownOutcome),
            _ => return Err(Error::Unavailable),
        }
        self.release().await
    }
    /// ADR-007 D7.6 (A1): COMMIT of a SERIALIZABLE transaction. PostgreSQL
    /// reports 40001 only for a transaction it has aborted, so `Ok(false)` is a
    /// definite rollback and the connection is reset and reused as usual. Any
    /// other COMMIT failure, a transport loss included, is `UnknownOutcome`.
    pub async fn commit_serializable(self, transaction_deadline: Instant) -> Result<bool> {
        // APP-010 review m1: the definition campaign pauses here to doom the
        // transaction through SSI (40001 at COMMIT) or to end its backend.
        #[cfg(test)]
        self.checkpoint("commit").await;
        let committed =
            match timeout_at(transaction_deadline, self.client().batch_execute("COMMIT")).await {
                Ok(Ok(())) => true,
                Ok(Err(error))
                    if error.code()
                        == Some(&tokio_postgres::error::SqlState::T_R_SERIALIZATION_FAILURE) =>
                {
                    false
                }
                _ => return Err(Error::UnknownOutcome),
            };
        self.release().await?;
        Ok(committed)
    }
    async fn release(mut self) -> Result<()> {
        #[cfg(test)]
        self.checkpoint("finish").await;
        let role = self.role();
        let object = self.object.as_mut().expect("owned lease");
        timeout_at(Instant::now() + CLEANUP, async {
            reset(object, role).await?;
            #[cfg(test)]
            self.owner.manager.settings.hooks.checkpoint("reset").await;
            Ok::<(), Error>(())
        })
        .await
        .map_err(|_| Error::Deadline)??;
        let current = self.owner.manager.configuration()?;
        if current.generation == object.generation && current.file_sha256 == object.fingerprint {
            // Only this path hands the object back to Deadpool.
            drop(self.object.take());
        }
        Ok(())
    }
}

async fn verify_identity(client: &Client, role: Role) -> Result<()> {
    let row = client.query_one("SELECT session_user::pg_catalog.text,current_user::pg_catalog.text,pg_catalog.current_database(),rolsuper,rolcreatedb,rolcreaterole,rolreplication,rolbypassrls,rolcanlogin,rolinherit,pg_catalog.pg_has_role(current_user,'rds_iam','MEMBER'),(SELECT ssl FROM pg_catalog.pg_stat_ssl WHERE pid=pg_catalog.pg_backend_pid()),pg_catalog.current_setting('session_replication_role'),pg_catalog.current_setting('search_path') FROM pg_catalog.pg_roles WHERE rolname=current_user", &[])
        .await.map_err(|_| Error::Unavailable)?;
    let text = |index| {
        row.try_get::<_, String>(index)
            .map_err(|_| Error::Unavailable)
    };
    let boolean = |index| {
        row.try_get::<_, bool>(index)
            .map_err(|_| Error::Unavailable)
    };
    if text(0)? != role.name()
        || text(1)? != role.name()
        || text(2)? != "genui"
        || (3..8).any(|index| boolean(index) != Ok(false))
        || !boolean(8)?
        || boolean(9)?
        || !boolean(10)?
        || !boolean(11)?
        || text(12)? != "origin"
        || text(13)? != "pg_catalog, pg_temp"
    {
        return Err(Error::Unavailable);
    }
    let rows = client.query("SELECT rolname FROM pg_catalog.pg_roles WHERE rolname NOT IN (current_user,'rds_iam') AND pg_catalog.pg_has_role(current_user,oid,'MEMBER') UNION ALL SELECT 'unsafe_membership' FROM pg_catalog.pg_auth_members WHERE member=(SELECT oid FROM pg_catalog.pg_roles WHERE rolname=current_user) AND (admin_option OR inherit_option)", &[])
        .await.map_err(|_| Error::Unavailable)?;
    if !rows.is_empty() {
        return Err(Error::Unavailable);
    }
    Ok(())
}

async fn verify_schema_version(client: &Client) -> Result<()> {
    let row = client
        .query_one(
            "SELECT version FROM genui_meta.schema_version WHERE singleton",
            &[],
        )
        .await
        .map_err(|_| Error::Unavailable)?;
    if row.try_get::<_, i32>(0).map_err(|_| Error::Unavailable)? != 10 {
        return Err(Error::Unavailable);
    }
    Ok(())
}

const BASELINE: &str = "SET search_path=pg_catalog,pg_temp; SET row_security=on; SET statement_timeout=2000; SET lock_timeout=1000; SET idle_in_transaction_session_timeout=3000; SELECT pg_catalog.set_config('genui.organization_id','',false),pg_catalog.set_config('genui.principal_id','',false),pg_catalog.set_config('genui.membership_epoch','',false)";
async fn reset(connection: &mut Connection, role: Role) -> Result<()> {
    connection.clean = false;
    // Replication mode can disable FK triggers. Refuse drift before DISCARD
    // ALL could mask it; never grant or use privileged SET to repair it.
    let replication_role: String = connection
        .client
        .query_one(
            "SELECT pg_catalog.current_setting('session_replication_role')",
            &[],
        )
        .await
        .map_err(|_| Error::Unavailable)?
        .try_get(0)
        .map_err(|_| Error::Unavailable)?;
    if replication_role != "origin" {
        return Err(Error::Unavailable);
    }
    connection
        .client
        .batch_execute("DISCARD ALL")
        .await
        .map_err(|_| Error::Unavailable)?;
    connection
        .client
        .batch_execute(BASELINE)
        .await
        .map_err(|_| Error::Unavailable)?;
    verify_baseline(&connection.client, role).await?;
    connection.clean = true;
    Ok(())
}
async fn verify_baseline(client: &Client, role: Role) -> Result<()> {
    let row = client.query_one("SELECT session_user::pg_catalog.text,current_user::pg_catalog.text,pg_catalog.current_setting('role'),pg_catalog.current_setting('row_security'),pg_catalog.current_setting('search_path'),pg_catalog.current_setting('genui.organization_id',true),pg_catalog.current_setting('genui.principal_id',true),pg_catalog.current_setting('genui.membership_epoch',true),pg_catalog.current_setting('session_replication_role')", &[])
        .await.map_err(|_| Error::Unavailable)?;
    for (index, expected) in [
        role.name(),
        role.name(),
        "none",
        "on",
        "pg_catalog, pg_temp",
        "",
        "",
        "",
        "origin",
    ]
    .into_iter()
    .enumerate()
    {
        if row
            .try_get::<_, String>(index)
            .map_err(|_| Error::Unavailable)?
            != expected
        {
            return Err(Error::Unavailable);
        }
    }
    Ok(())
}

pub struct Workspace {
    pub(super) inner: Arc<RolePool>,
}
pub struct Control {
    pub(super) inner: Arc<RolePool>,
}
pub struct Data {
    pub(super) inner: Arc<RolePool>,
}
/// Worker business authorization stays unavailable until its owning job contract.
pub struct Worker {
    pub(super) inner: Arc<RolePool>,
}
pub struct Stores {
    pub workspace: Workspace,
    pub control: Control,
    pub data: Data,
    pub worker: Worker,
}
impl Stores {
    pub fn new(settings: Settings) -> Result<Self> {
        let registry = Arc::new(Registry::load(&settings.registry)?);
        let catalog = Arc::new(Catalog::embedded()?);
        let mut bytes = Vec::new();
        std::fs::File::open(&settings.ca)
            .map_err(|_| Error::Configuration)?
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::Configuration)?;
        if bytes.len() > 1024 * 1024 {
            return Err(Error::Configuration);
        }
        let mut roots = rustls::RootCertStore::empty();
        for cert in CertificateDer::pem_slice_iter(&bytes) {
            roots
                .add(cert.map_err(|_| Error::Configuration)?)
                .map_err(|_| Error::Configuration)?;
        }
        if roots.is_empty() {
            return Err(Error::Configuration);
        }
        let config = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_protocol_versions(&[&rustls::version::TLS13, &rustls::version::TLS12])
        .map_err(|_| Error::Configuration)?
        .with_root_certificates(roots)
        .with_no_client_auth();
        let tls = MakeRustlsConnect::new(config);
        let settings = Arc::new(settings);
        let create = |role| -> Result<Arc<RolePool>> {
            let manager = Arc::new(RoleManager {
                settings: settings.clone(),
                tls: tls.clone(),
                catalog: catalog.clone(),
                role,
            });
            let pool = Pool::builder(manager.as_ref().clone())
                .max_size(2)
                .build()
                .map_err(|_| Error::Configuration)?;
            Ok(Arc::new(RolePool {
                pool,
                manager,
                registry: registry.clone(),
            }))
        };
        Ok(Self {
            workspace: Workspace {
                inner: create(Role::Workspace)?,
            },
            control: Control {
                inner: create(Role::Control)?,
            },
            data: Data {
                inner: create(Role::Data)?,
            },
            worker: Worker {
                inner: create(Role::Worker)?,
            },
        })
    }
    pub fn registry_identity(&self) -> &RegistryIdentity {
        &self.workspace.inner.registry.identity
    }
    /// Fixed-role database readiness only. Storage/KMS and user admission have
    /// separate owners. This probe never installs tenant scope or returns data.
    pub async fn readiness(&self) -> Result<()> {
        async fn probe(pool: &Arc<RolePool>) -> Result<()> {
            let lease = Lease::acquire(pool).await?;
            verify_baseline(lease.client(), lease.role()).await?;
            pool.manager.verify_admission(lease.client()).await?;
            lease.finish(false, Instant::now() + TRANSACTION).await
        }
        timeout_at(Instant::now() + ACQUIRE, async {
            tokio::try_join!(
                probe(&self.workspace.inner),
                probe(&self.control.inner),
                probe(&self.data.inner),
                probe(&self.worker.inner)
            )?;
            Ok(())
        })
        .await
        .map_err(|_| Error::Deadline)?
    }
    pub async fn close(&self) -> Result<()> {
        for pool in [
            &self.workspace.inner,
            &self.control.inner,
            &self.data.inner,
            &self.worker.inner,
        ] {
            pool.pool.close();
        }
        timeout_at(Instant::now() + CLEANUP, async {
            while capacity().all.available_permits() != 8 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .map_err(|_| Error::Deadline)
    }
}

#[cfg(test)]
pub(super) fn slots() -> (usize, [usize; 4]) {
    (
        8 - capacity().all.available_permits(),
        std::array::from_fn(|i| 2 - capacity().roles[i].available_permits()),
    )
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    struct Transport {
        slots: Arc<Semaphore>,
        observed: Arc<std::sync::atomic::AtomicUsize>,
    }
    impl Future for Transport {
        type Output = std::result::Result<(), tokio_postgres::Error>;
        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
            Poll::Pending
        }
    }
    impl Drop for Transport {
        fn drop(&mut self) {
            self.observed
                .store(self.slots.available_permits(), Ordering::SeqCst);
        }
    }
    #[tokio::test]
    async fn abort_request_retains_permits_until_transport_drop() {
        let slots = Arc::new(Semaphore::new(1));
        let roles = Arc::new(Semaphore::new(1));
        let observed = Arc::new(std::sync::atomic::AtomicUsize::new(99));
        let permits = DriverPermits {
            _all: slots.clone().acquire_owned().await.unwrap(),
            _role: roles.clone().acquire_owned().await.unwrap(),
        };
        let driver = Driver::start(
            Transport {
                slots: slots.clone(),
                observed: observed.clone(),
            },
            permits,
        );
        let closed = driver._closed.clone();
        drop(driver);
        // Single-thread runtime: requesting abort cannot have polled/destructed
        // the owned driver until this task yields.
        assert_eq!(slots.available_permits(), 0);
        assert!(!closed.value.load(Ordering::Acquire));
        timeout_at(Instant::now() + CLEANUP, async {
            while !closed.value.load(Ordering::Acquire) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(observed.load(Ordering::SeqCst), 0);
        assert_eq!(slots.available_permits(), 1);
        assert_eq!(roles.available_permits(), 1);
    }
}
