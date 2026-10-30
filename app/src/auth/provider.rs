//! The fAuth back channel (ADR-006; the hash-bound contract in
//! evidence/APP-006/upstream-auth-source): PAR, the authorize hand-off URL,
//! the code exchange, refresh, logout and the resend of the verification
//! mail. Bounded: 1 s connect, 3 s total, no redirects, retries, proxies or
//! compression, 32 KiB bodies, `deny_unknown_fields` on every parsed shape.
//! No response JSON, header or `Set-Cookie` is ever returned to a browser:
//! callers receive typed values or fixed outcome codes only.
use axum::http::{header, StatusCode};
use reqwest::{redirect, Client as Http};
use serde::Deserialize;
use std::{fmt, time::Duration};

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(1);
pub const TOTAL_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_BODY: usize = 32 * 1024;
const REQUEST_URI_PREFIX: &str = "urn:ietf:params:oauth:request_uri:";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Par {
    request_uri: String,
    expires_in: u64,
}

/// The exact token-response shape of the pinned contract
/// (`crates/server/src/handlers/token.rs:306-324`).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Token {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub expires_at: u64,
    pub user: User,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct User {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
    pub avatar: Option<String>,
    /// Backend metadata, not a JWT claim; GenUI's activation gate.
    pub email_verified: bool,
    pub roles: Vec<String>,
    pub auth_method: String,
    pub mfa_verified: bool,
}
impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Token([redacted])")
    }
}

/// Fixed refusal codes; never a body, token, URL or provider message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Failure {
    Transport,
    Status,
    Contract,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefreshFailure {
    /// 401 (`invalid_grant`, `token_reuse_detected`, `session_expired`).
    Unauthorized,
    /// Anything else, including a timeout: the presented token may have been
    /// consumed, so it is never presented again.
    Failed,
}

/// ADR-006 resend mapping: two sanitized results, plus the two outcomes that
/// send the browser to a fresh sign-in. `status` and `detail` are retained
/// server-side only (sanitized evidence), never shown or forwarded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Resend {
    /// `202 status=provider_accepted`: a new message may arrive (not delivery).
    Requested,
    /// `200 status=already_verified`: a fresh sign-in sets the metadata.
    AlreadyVerified,
    /// `429`/`503`/`502`, a timeout, and every other answer, including the
    /// rehearsal image's `200 {message}` and `500 email_failed` (dfba787).
    Unavailable { status: u16, detail: &'static str },
    /// `401` or `404`: destroy the pre-access record, fresh sign-in.
    SignInAgain { status: u16 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Logout {
    /// `204`. Not the negative receipt: a later refresh must fail (ADR-006).
    Acknowledged,
    Failed,
}

pub struct Client {
    origin: String,
    http: Http,
}
impl Client {
    /// Production: an exact `https://host[:port]` origin only.
    pub fn new(origin: &str) -> Option<Self> {
        let parsed = url::Url::parse(origin).ok()?;
        if parsed.scheme() != "https" {
            return None;
        }
        Self::build(origin, &parsed)
    }
    /// The cfg(test) synthetic issuer: plain HTTP on a loopback address only.
    #[cfg(test)]
    pub fn loopback(origin: &str) -> Option<Self> {
        let parsed = url::Url::parse(origin).ok()?;
        let loopback = parsed
            .host_str()
            .and_then(|h| h.parse::<std::net::IpAddr>().ok())
            .is_some_and(|ip| ip.is_loopback());
        if parsed.scheme() != "http" || !loopback {
            return None;
        }
        Self::build(origin, &parsed)
    }
    fn build(origin: &str, parsed: &url::Url) -> Option<Self> {
        if parsed.origin().ascii_serialization() != origin
            || !parsed.username().is_empty()
            || parsed.password().is_some()
        {
            return None;
        }
        let http = Http::builder()
            .redirect(redirect::Policy::none())
            .retry(reqwest::retry::never())
            .no_proxy()
            .no_gzip()
            .no_brotli()
            .no_zstd()
            .no_deflate()
            .referer(false)
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(TOTAL_TIMEOUT)
            .build()
            .ok()?;
        Some(Self {
            origin: origin.to_owned(),
            http,
        })
    }
    fn endpoint(&self, slug: &str, operation: &str) -> String {
        format!("{}/t/{slug}/{operation}", self.origin)
    }

    /// `POST /t/{slug}/oauth/par`, form-encoded; `201 {request_uri, expires_in: 60}`.
    /// The confidential `invite_token` (APP-074) travels in this body only.
    #[allow(clippy::too_many_arguments)]
    pub async fn par(
        &self,
        slug: &str,
        client_id: &str,
        redirect_uri: &str,
        state: &str,
        challenge: &str,
        invite: Option<&str>,
    ) -> Result<String, Failure> {
        let mut pairs = vec![
            ("provider", "email"),
            ("client_id", client_id),
            ("redirect_uri", redirect_uri),
            ("response_type", "code"),
            ("state", state),
            ("code_challenge", challenge),
            ("code_challenge_method", "S256"),
        ];
        if let Some(invite) = invite {
            pairs.push(("invite_token", invite));
        }
        let body = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(&pairs)
            .finish();
        let response = bounded(
            self.http
                .post(self.endpoint(slug, "oauth/par"))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(body)
                .send(),
        )
        .await?;
        let par: Par = json(response, StatusCode::CREATED).await?;
        let suffix = par
            .request_uri
            .strip_prefix(REQUEST_URI_PREFIX)
            .ok_or(Failure::Contract)?;
        if par.expires_in != 60
            || suffix.len() != 32
            || !suffix
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Failure::Contract);
        }
        Ok(par.request_uri)
    }

    /// The GET that consumes the single-use `request_uri` and creates fAuth's
    /// five-minute browser-bound hosted request.
    pub fn authorize_url(&self, slug: &str, client_id: &str, request_uri: &str) -> String {
        let mut url = url::Url::parse(&self.endpoint(slug, "auth/authorize")).expect("origin");
        url.query_pairs_mut()
            .append_pair("client_id", client_id)
            .append_pair("request_uri", request_uri);
        url.into()
    }

    /// `POST /t/{slug}/auth/token`, JSON `grant_type=authorization_code`.
    pub async fn exchange(
        &self,
        slug: &str,
        client_id: &str,
        redirect_uri: &str,
        code: &str,
        verifier: &str,
    ) -> Result<Token, Failure> {
        let response = bounded(
            self.http
                .post(self.endpoint(slug, "auth/token"))
                .json(&serde_json::json!({
                    "grant_type": "authorization_code", "code": code,
                    "code_verifier": verifier, "client_id": client_id,
                    "redirect_uri": redirect_uri,
                }))
                .send(),
        )
        .await?;
        json(response, StatusCode::OK).await
    }

    /// `POST /t/{slug}/auth/token`, JSON `grant_type=refresh_token`. The
    /// caller serializes per session and never presents a token again after
    /// any answer but success.
    pub async fn refresh(&self, slug: &str, refresh_token: &str) -> Result<Token, RefreshFailure> {
        let response = bounded(
            self.http
                .post(self.endpoint(slug, "auth/token"))
                .json(&serde_json::json!({"grant_type": "refresh_token", "refresh_token": refresh_token}))
                .send(),
        )
        .await
        .map_err(|_| RefreshFailure::Failed)?;
        if response.status() == StatusCode::UNAUTHORIZED {
            return Err(RefreshFailure::Unauthorized);
        }
        json(response, StatusCode::OK)
            .await
            .map_err(|_| RefreshFailure::Failed)
    }

    /// `POST /t/{slug}/auth/logout`, JSON `{refresh_token}`; `204`.
    pub async fn logout(&self, slug: &str, refresh_token: &str) -> Logout {
        match bounded(
            self.http
                .post(self.endpoint(slug, "auth/logout"))
                .json(&serde_json::json!({ "refresh_token": refresh_token }))
                .send(),
        )
        .await
        {
            Ok(response) if response.status() == StatusCode::NO_CONTENT => Logout::Acknowledged,
            _ => Logout::Failed,
        }
    }

    /// `POST /t/{slug}/auth/resend-verification` with the account's bearer.
    pub async fn resend(&self, slug: &str, access_token: &str) -> Resend {
        let response = match bounded(
            self.http
                .post(self.endpoint(slug, "auth/resend-verification"))
                .bearer_auth(access_token)
                .send(),
        )
        .await
        {
            Ok(response) => response,
            Err(_) => {
                return Resend::Unavailable {
                    status: 0,
                    detail: "timeout_or_transport",
                }
            }
        };
        let status = response.status().as_u16();
        if matches!(status, 401 | 404) {
            return Resend::SignInAgain { status };
        }
        let body: Option<serde_json::Value> = json(response, response_status(status)).await.ok();
        let field = |name: &str| {
            body.as_ref()
                .and_then(|b| b.get(name))
                .and_then(|v| v.as_str())
                .map(str::to_owned)
        };
        match (
            status,
            field("status").as_deref(),
            field("error").as_deref(),
        ) {
            (202, Some("provider_accepted"), _) => Resend::Requested,
            (200, Some("already_verified"), _) => Resend::AlreadyVerified,
            (429, _, Some("resend_cooldown")) => Resend::Unavailable {
                status,
                detail: "resend_cooldown",
            },
            (429, _, Some("too_many_requests")) => Resend::Unavailable {
                status,
                detail: "too_many_requests",
            },
            (503, _, Some("email_unavailable")) => Resend::Unavailable {
                status,
                detail: "email_unavailable",
            },
            (502, _, Some("email_failed")) => Resend::Unavailable {
                status,
                detail: "email_failed",
            },
            _ => Resend::Unavailable {
                status,
                detail: "unmapped",
            },
        }
    }
}

fn response_status(status: u16) -> StatusCode {
    StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
}

async fn bounded(
    request: impl std::future::Future<Output = reqwest::Result<reqwest::Response>>,
) -> Result<reqwest::Response, Failure> {
    tokio::time::timeout(TOTAL_TIMEOUT, request)
        .await
        .map_err(|_| Failure::Transport)?
        .map_err(|_| Failure::Transport)
}

/// A bounded `application/json` body of exactly the expected status.
async fn json<T: serde::de::DeserializeOwned>(
    mut response: reqwest::Response,
    expected: StatusCode,
) -> Result<T, Failure> {
    if response.status() != expected {
        return Err(Failure::Status);
    }
    if response.headers().contains_key(header::CONTENT_ENCODING)
        || response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_none_or(|v| v.split(';').next().map(str::trim) != Some("application/json"))
        || response
            .content_length()
            .is_some_and(|n| n > MAX_BODY as u64)
    {
        return Err(Failure::Contract);
    }
    let mut body = Vec::new();
    let read = async {
        while let Some(chunk) = response.chunk().await.map_err(|_| Failure::Transport)? {
            if body.len() + chunk.len() > MAX_BODY {
                return Err(Failure::Contract);
            }
            body.extend_from_slice(&chunk);
        }
        Ok(())
    };
    tokio::time::timeout(TOTAL_TIMEOUT, read)
        .await
        .map_err(|_| Failure::Transport)??;
    serde_json::from_slice(&body).map_err(|_| Failure::Contract)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_requires_an_exact_https_origin() {
        assert!(Client::new("https://auth-staging.getforma.dev").is_some());
        for bad in [
            "http://auth-staging.getforma.dev",
            "https://auth-staging.getforma.dev/",
            "https://user@auth-staging.getforma.dev",
            "https://auth-staging.getforma.dev/path",
            "http://127.0.0.1:9",
        ] {
            assert!(Client::new(bad).is_none(), "{bad}");
        }
        assert!(Client::loopback("http://127.0.0.1:9").is_some());
        assert!(Client::loopback("http://auth.example.test").is_none());
        let client = Client::new("https://auth-staging.getforma.dev").unwrap();
        assert_eq!(
            client.authorize_url("northstar", "northstar-platform", "urn:ietf:params:oauth:request_uri:0123456789abcdef0123456789abcdef"),
            "https://auth-staging.getforma.dev/t/northstar/auth/authorize?client_id=northstar-platform&request_uri=urn%3Aietf%3Aparams%3Aoauth%3Arequest_uri%3A0123456789abcdef0123456789abcdef"
        );
    }
}
