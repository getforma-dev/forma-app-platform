//! Signed-in personas without a sign-in provider (ADR-012 D12.1). The
//! session cookie is a `__Host-` cookie, which a browser never keeps from
//! plain http, so each persona gets its own loopback listener whose layer
//! puts that persona's session on every request (what the retired Python
//! proxies did, in process, so nothing can run stale code).
//!
//! Each browser gets its own session of the persona (a browser keeps one
//! live stream per session; two browsers on one session replaced each
//! other's). A browser is told apart by a `genui_dev_client` cookie set on
//! its first page load; a request with no such cookie (curl, a probe) uses
//! the persona's last session, so it never takes a browser's. On the clean
//! profile the first page a browser loads from a new database (after `just
//! dev clean reset`) carries `Clear-Site-Data: "storage"`, so the canvas the
//! browser kept for that origin starts empty too.
use super::{Persona, BROWSER_SLOTS};
use crate::{auth::Auth, storage::Stores};
use axum::{
    body::Body,
    extract::{ConnectInfo, Request, State},
    http::{header, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::Response,
    Router,
};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex, RwLock},
    time::Duration,
};
use tower::ServiceExt;

const CLIENT_COOKIE: &str = "genui_dev_client";
const EPOCH_COOKIE: &str = "genui_dev_epoch";
/// How often each session is touched (idle limit: 1 hour).
const KEEPALIVE: Duration = Duration::from_secs(60);

/// Mints a member session the way the command campaign does: a vault entry
/// and the session row; everything a request then does is the product path.
#[derive(Clone)]
pub(crate) struct Minter {
    pub auth: Arc<Auth>,
    pub stores: Arc<Stores>,
    pub organization: &'static str,
}
impl Minter {
    pub async fn mint(&self, principal: &str) -> Result<String, String> {
        let token = crate::auth::provider::Token {
            access_token: "synthetic-access".into(),
            refresh_token: crate::auth::capability::random_capability()
                .ok_or("random capability")?,
            token_type: "Bearer".into(),
            expires_in: 900,
            expires_at: crate::auth::now() + 900,
            user: crate::auth::provider::User {
                id: format!("usr_{principal}"),
                email: "dev@example.test".into(),
                name: None,
                avatar: None,
                email_verified: true,
                roles: Vec::new(),
                auth_method: "password".into(),
                mfa_verified: false,
            },
        };
        let reference = self
            .auth
            .vault
            .insert("northstar", &token)
            .ok_or("the session vault is full")?;
        self.stores
            .workspace
            .mint_session_for_tests(self.organization, principal, &reference)
            .await
            .map_err(|e| format!("minting a session: {e}"))
    }
}

/// One persona's listener state.
struct Gate {
    persona: usize,
    sessions: Arc<RwLock<Vec<Vec<String>>>>,
    clients: Mutex<HashMap<String, usize>>,
    /// Set on the clean profile: the database's epoch.
    clear_on_reset: Option<String>,
}

fn cookies(request: &Request) -> Vec<(String, String)> {
    request
        .headers()
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| {
            let (name, value) = pair.trim().split_once('=')?;
            Some((name.to_owned(), value.to_owned()))
        })
        .collect()
}

fn random_id() -> String {
    let mut bytes = [0u8; 12];
    let _ = ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn accepts_html(request: &Request) -> bool {
    request
        .headers()
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.contains("text/html"))
}

async fn persona_layer(
    State(gate): State<Arc<Gate>>,
    mut request: Request,
    next: Next,
) -> Response {
    let jar = cookies(&request);
    let page = request.method() == Method::GET && accepts_html(&request);
    let known = jar
        .iter()
        .find(|(name, value)| {
            name == CLIENT_COOKIE
                && value.len() <= 64
                && value.bytes().all(|b| b.is_ascii_alphanumeric())
        })
        .map(|(_, value)| value.clone());
    // A browser (it has our cookie, or this is its first page load) gets its
    // own slot; anything else shares the last session.
    let (client, new_client) = match known {
        Some(id) => (Some(id), false),
        None if page => (Some(random_id()), true),
        None => (None, false),
    };
    let session = {
        let sessions = gate.sessions.read().expect("sessions lock");
        let slots = &sessions[gate.persona];
        let index = client.as_ref().map(|id| {
            let mut clients = gate.clients.lock().expect("clients lock");
            let next = clients.len() % (BROWSER_SLOTS - 1);
            *clients.entry(id.clone()).or_insert(next)
        });
        slots[index.unwrap_or(BROWSER_SLOTS - 1)].clone()
    };
    let mut kept: Vec<String> = jar
        .iter()
        .filter(|(name, _)| name != crate::auth::cookies::SESSION)
        .map(|(name, value)| format!("{name}={value}"))
        .collect();
    kept.push(format!("{}={session}", crate::auth::cookies::SESSION));
    let headers = request.headers_mut();
    headers.remove(header::COOKIE);
    if let Ok(value) = HeaderValue::from_str(&kept.join("; ")) {
        headers.insert(header::COOKIE, value);
    }
    let stale = gate.clear_on_reset.as_ref().filter(|epoch| {
        page && request
            .headers()
            .get("sec-fetch-dest")
            .is_none_or(|v| v.as_bytes() == b"document")
            && !jar
                .iter()
                .any(|(name, value)| name == EPOCH_COOKIE && value == *epoch)
    });
    let stale = stale.cloned();
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    if let (true, Some(id)) = (new_client, client) {
        if let Ok(value) = HeaderValue::from_str(&format!(
            "{CLIENT_COOKIE}={id}; Path=/; SameSite=Lax; HttpOnly"
        )) {
            headers.append(header::SET_COOKIE, value);
        }
    }
    if let Some(epoch) = stale {
        headers.insert("clear-site-data", HeaderValue::from_static("\"storage\""));
        if let Ok(value) =
            HeaderValue::from_str(&format!("{EPOCH_COOKIE}={epoch}; Path=/; SameSite=Lax"))
        {
            headers.append(header::SET_COOKIE, value);
        }
    }
    response
}

/// A request through the router in process (the keepalive's member read).
async fn status(app: &Router, session: &str) -> StatusCode {
    let mut request = axum::http::Request::builder()
        .method("GET")
        .uri("/v1/members")
        .header(
            header::COOKIE,
            format!("{}={session}", crate::auth::cookies::SESSION),
        )
        .body(Body::empty())
        .expect("request");
    request.extensions_mut().insert(ConnectInfo(
        "127.0.0.1:4000".parse::<SocketAddr>().expect("address"),
    ));
    match app.clone().oneshot(request).await {
        Ok(response) => response.status(),
        Err(never) => match never {},
    }
}

/// Binds one loopback listener per persona and keeps the sessions alive;
/// returns each persona's origin.
pub(crate) async fn serve(
    app: Router,
    minter: Minter,
    personas: &'static [Persona],
    ports: &[u16],
    sessions: Vec<Vec<String>>,
    clear_on_reset: Option<String>,
) -> Result<Vec<String>, String> {
    let sessions = Arc::new(RwLock::new(sessions));
    let mut origins = Vec::new();
    for (index, port) in ports.iter().enumerate() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", *port))
            .await
            .map_err(|e| format!("127.0.0.1:{port} ({}): {e}", personas[index].name))?;
        origins.push(format!("http://127.0.0.1:{port}"));
        let gate = Arc::new(Gate {
            persona: index,
            sessions: sessions.clone(),
            clients: Mutex::new(HashMap::new()),
            clear_on_reset: clear_on_reset.clone(),
        });
        let served = app
            .clone()
            .layer(middleware::from_fn_with_state(gate, persona_layer));
        tokio::spawn(async move {
            let _ = axum::serve(
                listener,
                served.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await;
        });
    }
    // Keep each session inside its idle limit; one that ended (a sign-out in
    // the browser, the 12-hour limit) is minted again.
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(KEEPALIVE).await;
            let current = sessions.read().expect("sessions lock").clone();
            for (index, slots) in current.iter().enumerate() {
                for (slot, session) in slots.iter().enumerate() {
                    if status(&app, session).await == StatusCode::OK {
                        continue;
                    }
                    match minter.mint(personas[index].principal).await {
                        Ok(fresh) => {
                            sessions.write().expect("sessions lock")[index][slot] = fresh;
                            println!(
                                "genui-dev: session for {} minted again",
                                personas[index].name
                            );
                        }
                        Err(reason) => eprintln!("genui-dev: {reason}"),
                    }
                }
            }
        }
    });
    Ok(origins)
}
