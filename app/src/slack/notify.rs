//! The approver DM: on every `record.committed` and `app.shared` of the
//! event hub, from any origin (canvas, MCP, Slack), each linked member of the
//! event's organization is judged under its own authority: its link is
//! resolved again (`resolve_slack_link`: unrevoked, current epoch, live
//! entitlement, registry identity), the work's records are listed as that
//! member (`record_read`, RLS, the work fence), and every record waiting on
//! that member's decision (`resolve::pending`: the member holds
//! `record_write`, the record is at a declared `from`, the member did not file
//! it) not yet announced to it is DMed once, with Approve and Reject. A member
//! who cannot open the work learns nothing (the listing is `Denied`). The
//! canvas commits and then shares to the approver, so the DM normally follows
//! the share's `app.shared`. Recorded debt (APP-098 DS-61): the approver is
//! derived from write access, not stored, and the once-per-version set lives
//! in process memory.
use super::{flows, Slack};
use crate::{
    actions::resolve,
    events::{self, Kind},
};
use std::sync::Arc;
use tokio::sync::broadcast::error::RecvError;

/// Announcements remembered per process before the set is reset.
const MAX_NOTIFIED: usize = 8192;

/// Subscribes to the hub for the life of the process.
pub fn spawn(state: Arc<Slack>) {
    let mut receiver = events::hub().subscribe();
    tokio::spawn(async move {
        loop {
            match receiver.recv().await {
                Ok(announcement) => {
                    let event = announcement.event;
                    if matches!(event.kind, Kind::RecordCommitted | Kind::AppShared) {
                        let state = state.clone();
                        tokio::spawn(async move {
                            approvers(&state, &event.organization, &event.app, &event.work).await
                        });
                    }
                }
                Err(RecvError::Lagged(skipped)) => {
                    tracing::warn!(
                        event = "slack.notify_lagged",
                        skipped,
                        "approver notifier lagged"
                    );
                }
                Err(RecvError::Closed) => break,
            }
        }
    });
}

/// DMs every linked member of `organization` who now has a claim in `work`
/// waiting on its decision (module notes).
pub async fn approvers(state: &Slack, organization: &str, app: &str, work: &str) {
    let Some(stores) = state.auth.stores.as_ref() else {
        return;
    };
    let links = match stores.workspace.slack_links(organization).await {
        Ok(links) => links,
        Err(error) => {
            tracing::warn!(event = "slack.notify_unavailable", error = %error, "linked members not read");
            return;
        }
    };
    for (team, user, principal) in links {
        let Ok(Some(member)) = stores.workspace.resolve_slack_link(&team, &user).await else {
            continue;
        };
        if member.principal() != principal || member.organization() != organization {
            continue;
        }
        let Ok(listing) = stores.data.list_records(&member, app, work).await else {
            continue;
        };
        for record in resolve::pending(&listing, member.principal()) {
            let key = (
                organization.to_owned(),
                principal.clone(),
                record.record_id.clone(),
                record.version,
            );
            let first = match state.notified.lock() {
                Ok(mut notified) => {
                    if notified.len() >= MAX_NOTIFIED {
                        notified.clear();
                    }
                    notified.insert(key)
                }
                Err(_) => false,
            };
            if !first {
                continue;
            }
            let (text, blocks) = flows::approval_request(record, state);
            match state.api.direct(&user, &text, Some(blocks)).await {
                Ok(()) => tracing::info!(
                    event = "slack.approver_notified",
                    organization_id = organization,
                    principal_id = member.principal(),
                    app_id = app,
                    work_id = work,
                    "approver DM sent"
                ),
                Err(error) => tracing::warn!(
                    event = "slack.dm_failed",
                    code = error.code(),
                    "approver DM not sent"
                ),
            }
        }
    }
}
