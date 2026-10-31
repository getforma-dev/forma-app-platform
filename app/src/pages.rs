//! The binary's server-rendered APP-009/APP-013 pages (sign-in, invitation,
//! pending, denied, workspace and record), shared by `main.rs` and the local
//! dev binary (`genui::dev`, feature `dev-stand`) so both render the same pages.
use crate::auth::{
    self, DeniedView, InviteView, PendingView, ResendResult, SigninView, WorkspaceView,
};
use crate::commands::{Notice, RecordPage, RecordView};
use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use forma_ir::PageProps;
use forma_server::{PageHandle, PageResponse, PolicyExtensions, RenderCtx, RenderError};

// APP-009 (ADR-006) pages: typed props only; no page reads a request value.
// APP-074: the invitation entry never shows a submitted code back.
#[derive(PageProps)]
pub struct InviteProps {
    pub failed: bool,
    /// APP-074 review m4: the ADR-006 window that refused, named.
    pub limit_minute: bool,
    pub limit_day: bool,
    pub limit_service: bool,
    pub unavailable: bool,
    pub described_by: String,
}
#[derive(PageProps)]
pub struct SigninProps {
    pub action: String,
    pub has_action: bool,
    pub failed: bool,
}
#[derive(PageProps)]
pub struct PendingProps {
    pub csrf_resend: String,
    pub csrf_signout: String,
    pub resend_available: bool,
    pub cooldown_seconds: u32,
    pub held: bool,
    pub hours_remaining: u32,
    pub requested: bool,
    pub unavailable: bool,
}
#[derive(PageProps)]
pub struct DeniedProps {
    pub csrf_signout: String,
}
// APP-076 G9: the member's openable works, resolved by the auth route.
#[derive(PageProps)]
pub struct RecordLinkProps {
    pub id: String,
    pub label: String,
    pub href: String,
}
#[derive(PageProps)]
pub struct WorkspaceItemProps {
    pub id: String,
    pub title: String,
    pub href: String,
    /// ADR-011: the work's group on the canvas (`/canvas#g-{work}`).
    pub canvas: String,
    pub has_records: bool,
    pub records: Vec<RecordLinkProps>,
}
#[derive(PageProps)]
pub struct WorkspaceProps {
    pub csrf_signout: String,
    pub csrf_signout_all: String,
    pub has_items: bool,
    pub items: Vec<WorkspaceItemProps>,
}
// APP-013 record page (D7.16): typed rows and premise fields; the key and the
// token are minted per render by the route, never read from the request.
#[derive(PageProps)]
pub struct FieldRowProps {
    pub id: String,
    pub kind: String,
    pub value: String,
    pub input: String,
    pub spec: String,
    pub spec_input: String,
    pub options: String,
    pub nullable: bool,
    pub writable: bool,
}
#[derive(PageProps)]
pub struct RecordProps {
    pub app: String,
    pub work: String,
    pub collection: String,
    pub record_id: String,
    pub schema: String,
    pub schema_version: String,
    pub publication: String,
    pub work_version: String,
    pub version: String,
    pub exists: bool,
    pub fields: Vec<FieldRowProps>,
    pub command: String,
    pub action: String,
    pub can_write: bool,
    pub csrf: String,
    pub idempotency_key: String,
    pub committed: bool,
    pub replayed: bool,
    pub conflict: bool,
    pub stale: bool,
    pub in_flight: bool,
    pub busy: bool,
    pub refused: bool,
    pub unknown: bool,
}

/// The APP-009 pages, rendered as complete `PageResponse`s (fresh nonce,
/// CSP and `no-store` together). The auth module decides status and cookies.
pub struct SsrPages {
    /// The configured fAuth origin: the two anonymous forms' `POST` answers
    /// `303` there, and Chromium and WebKit judge `form-action` on every hop
    /// of a form submission's redirect chain (APP-009 review M1).
    pub auth_origin: Option<String>,
    pub invite: PageHandle<InviteProps>,
    pub signin: PageHandle<SigninProps>,
    pub pending: PageHandle<PendingProps>,
    pub denied: PageHandle<DeniedProps>,
    pub workspace: PageHandle<WorkspaceProps>,
    pub record: PageHandle<RecordProps>,
}
impl RecordPage for SsrPages {
    fn record(&self, view: &RecordView) -> Response {
        let notice = |expected: Notice| view.notice == Some(expected);
        respond(
            "/record",
            self.record.render(
                &RecordProps {
                    app: view.app.clone(),
                    work: view.work.clone(),
                    collection: view.collection.clone(),
                    record_id: view.record_id.clone(),
                    schema: view.schema.clone(),
                    schema_version: view.schema_version.clone(),
                    publication: view.publication.clone(),
                    work_version: view.work_version.clone(),
                    version: view.version.clone(),
                    exists: view.exists,
                    fields: view
                        .fields
                        .iter()
                        .map(|row| FieldRowProps {
                            id: row.id.clone(),
                            kind: row.kind.clone(),
                            value: row.value.clone(),
                            input: row.input.clone(),
                            spec: row.spec.clone(),
                            spec_input: row.spec_input.clone(),
                            options: row.options.clone(),
                            nullable: row.nullable,
                            writable: row.writable,
                        })
                        .collect(),
                    command: view.command.clone(),
                    action: view.action.clone(),
                    can_write: view.can_write,
                    csrf: view.csrf.clone(),
                    idempotency_key: view.idempotency_key.clone(),
                    committed: notice(Notice::Committed),
                    replayed: notice(Notice::Replayed),
                    conflict: notice(Notice::Conflict),
                    stale: notice(Notice::Stale),
                    in_flight: notice(Notice::InFlight),
                    busy: notice(Notice::Busy),
                    refused: notice(Notice::Refused),
                    unknown: notice(Notice::Unknown),
                },
                &RenderCtx::new().with_title("Record"),
            ),
        )
    }
}
impl SsrPages {
    /// A page whose form hands off to the auth origin: `form-action 'self'`
    /// plus that exact origin (forma-server `PolicyExtensions::form_action`).
    /// Without a configured origin the form fails neutrally and stays `'self'`.
    fn handoff(&self, title: &str) -> RenderCtx {
        let ctx = RenderCtx::new().with_title(title);
        match &self.auth_origin {
            Some(origin) => ctx.with_policy(PolicyExtensions::new().form_action(origin.clone())),
            None => ctx,
        }
    }
}
impl auth::Pages for SsrPages {
    fn invite(&self, view: &InviteView) -> Response {
        respond(
            "/invite",
            self.invite.render(
                &InviteProps {
                    failed: view.failed,
                    limit_minute: view.limited == Some(auth::InviteLimit::Minute),
                    limit_day: view.limited == Some(auth::InviteLimit::Day),
                    limit_service: view.limited == Some(auth::InviteLimit::Service),
                    unavailable: view.unavailable,
                    described_by: if view.failed || view.limited.is_some() || view.unavailable {
                        "invite-error"
                    } else {
                        "invite-help"
                    }
                    .into(),
                },
                &self.handoff("Enter your invitation code"),
            ),
        )
    }
    fn signin(&self, view: &SigninView) -> Response {
        respond(
            "/signin",
            self.signin.render(
                &SigninProps {
                    action: view.action.clone().unwrap_or_default(),
                    has_action: view.action.is_some(),
                    failed: view.failed,
                },
                &self.handoff("Sign in"),
            ),
        )
    }
    fn pending(&self, view: &PendingView) -> Response {
        respond(
            "/auth/pending",
            self.pending.render(
                &PendingProps {
                    csrf_resend: view.csrf_resend.clone(),
                    csrf_signout: view.csrf_signout.clone(),
                    resend_available: view.resend_available,
                    cooldown_seconds: view.cooldown_seconds,
                    held: view.hours_remaining > 0,
                    hours_remaining: view.hours_remaining,
                    requested: view.result == Some(ResendResult::Requested),
                    unavailable: view.result == Some(ResendResult::Unavailable),
                },
                &RenderCtx::new().with_title("Confirm your email address"),
            ),
        )
    }
    fn denied(&self, view: &DeniedView) -> Response {
        respond(
            "/auth/callback",
            self.denied.render(
                &DeniedProps {
                    csrf_signout: view.csrf_signout.clone(),
                },
                &RenderCtx::new().with_title("No access"),
            ),
        )
    }
    fn workspace(&self, view: &WorkspaceView) -> Response {
        respond(
            "/workspace",
            self.workspace.render(
                &WorkspaceProps {
                    csrf_signout: view.csrf_signout.clone(),
                    csrf_signout_all: view.csrf_signout_all.clone(),
                    has_items: !view.items.is_empty(),
                    items: view
                        .items
                        .iter()
                        .map(|item| WorkspaceItemProps {
                            id: item.href.clone(),
                            title: item.title.clone(),
                            href: item.href.clone(),
                            canvas: format!(
                                "/canvas#g-{}",
                                item.href.rsplit('/').next().unwrap_or_default()
                            ),
                            has_records: !item.records.is_empty(),
                            records: item
                                .records
                                .iter()
                                .map(|record| RecordLinkProps {
                                    id: record.href.clone(),
                                    label: record.label.clone(),
                                    href: record.href.clone(),
                                })
                                .collect(),
                        })
                        .collect(),
                },
                &RenderCtx::new().with_title("Workspace"),
            ),
        )
    }
}

/// A `PageResponse` sends the HTML with the policy for its nonce,
/// `Cache-Control: no-store` and the document security headers. Forma logs
/// nothing: a failure is counted here by code and route and answered with our
/// own error, never an empty shell.
pub fn respond(route: &str, rendered: Result<PageResponse, RenderError>) -> Response {
    match rendered {
        Ok(page) => page.into_response(),
        Err(error) => {
            tracing::error!(code = error.code(), route, "{error}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [
                    (header::CACHE_CONTROL, "no-store"),
                    (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
                ],
                "Internal Server Error",
            )
                .into_response()
        }
    }
}
