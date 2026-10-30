//! Demo shortcuts (ADR-011 pitch; Victor, 2026-10-30: "where something is
//! limited, make the demo look polished with a clearly-scoped shortcut").
//!
//! Every shortcut in this service is keyed off two environment switches and
//! an organization check, so that nothing here can reach a real beta
//! organization: `GENUI_DEMO_MODE` (`1`/`true`; off by default) turns the
//! shortcuts on, and `GENUI_DEMO_ORGANIZATIONS` (comma-separated organization
//! ids) names the synthetic demo organizations they apply to. A request whose
//! organization is not listed behaves exactly as without demo mode. Each
//! shortcut is registered in ticket APP-098 ("Demo shortcuts register") with
//! what it replaces and its proper fix; the audited commit, the permission
//! model, the receipts and the live updates are never shortcut.
use std::sync::{OnceLock, RwLock};

pub const MODE_ENV: &str = "GENUI_DEMO_MODE";
pub const ORGANIZATIONS_ENV: &str = "GENUI_DEMO_ORGANIZATIONS";

struct Settings {
    enabled: bool,
    organizations: Vec<String>,
}

static SETTINGS: OnceLock<RwLock<Settings>> = OnceLock::new();

fn settings() -> &'static RwLock<Settings> {
    SETTINGS.get_or_init(|| RwLock::new(from_env()))
}

fn from_env() -> Settings {
    let enabled = std::env::var(MODE_ENV)
        .map(|value| matches!(value.trim(), "1" | "true" | "on" | "yes"))
        .unwrap_or(false);
    let organizations = std::env::var(ORGANIZATIONS_ENV)
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|id| crate::storage::valid_identifier(id))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    Settings {
        enabled,
        organizations,
    }
}

/// Whether demo mode is on and `organization` is one of the listed demo
/// organizations. False for every real organization, always.
pub fn is_demo_org(organization: &str) -> bool {
    settings()
        .read()
        .map(|s| s.enabled && s.organizations.iter().any(|id| id == organization))
        .unwrap_or(false)
}

/// Tests and campaigns: set the switches without the environment.
#[cfg(test)]
pub fn configure_for_tests(enabled: bool, organizations: &[&str]) {
    if let Ok(mut current) = settings().write() {
        current.enabled = enabled;
        current.organizations = organizations.iter().map(|id| id.to_string()).collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_shortcuts_apply_to_listed_organizations_only() {
        let org = "00000000-0000-4000-8000-000000000001";
        let other = "00000000-0000-4000-8000-000000000002";
        configure_for_tests(false, &[org]);
        assert!(!is_demo_org(org), "off by default");
        configure_for_tests(true, &[org]);
        assert!(is_demo_org(org));
        assert!(!is_demo_org(other));
        assert!(!is_demo_org("not-an-id"));
        configure_for_tests(true, &[]);
        assert!(
            !is_demo_org(org),
            "no organization listed: no shortcut anywhere"
        );
        // Review: the composer's and the canvas's own lists go through this
        // switch too, so demo mode off turns every shortcut off.
        let composer =
            crate::composer::Composer::new(None, "r", "g").with_demo_organizations([org.into()]);
        let canvas: std::collections::BTreeSet<String> = [org.to_owned()].into();
        configure_for_tests(false, &[org]);
        assert!(!composer.demo_for(org));
        assert!(!crate::query::canvas::is_demo(&canvas, org));
        configure_for_tests(true, &[org]);
        assert!(composer.demo_for(org) && !composer.demo_for(other));
        assert!(crate::query::canvas::is_demo(&canvas, org));
        assert!(!crate::query::canvas::is_demo(&Default::default(), org));
        configure_for_tests(false, &[]);
    }
}
