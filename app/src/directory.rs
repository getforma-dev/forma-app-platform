//! Member display names (ADR-011 scenarios, APP-095 debt item 3: principals
//! carry no display name in the Phase 1 schema, so the members list showed
//! "Member <8 hex>" only).
//!
//! The minimal honest form for the pitch: an operator-maintained file named by
//! `GENUI_DISPLAY_NAMES_FILE`, JSON `{"members":[{"principal_id":"<uuid>",
//! "display_name":"Sarah Chen"}, …]}`, read once at startup. Fixtures and the
//! demo organization get synthetic names from their campaign; beta members
//! get a name only when the operator writes one. A name is a label for
//! people (the members list, the approver chip, the records list), never an
//! identity: authority still comes from the principal id and its grants.
//! Proper fix (APP-097/APP-098): a `display_name` column on `memberships`
//! written by the operator channel under D7.3, replacing this file.
use serde::Deserialize;
use std::{
    collections::HashMap,
    sync::{OnceLock, RwLock},
};

pub const FILE_ENV: &str = "GENUI_DISPLAY_NAMES_FILE";
/// A display name is short, printable, single-line text.
pub const MAX_NAME_CHARACTERS: usize = 80;
const MAX_FILE_BYTES: u64 = 256 * 1024;

#[derive(Deserialize)]
struct File {
    members: Vec<Entry>,
}
#[derive(Deserialize)]
struct Entry {
    principal_id: String,
    display_name: String,
}

static NAMES: OnceLock<RwLock<HashMap<String, String>>> = OnceLock::new();

fn names() -> &'static RwLock<HashMap<String, String>> {
    NAMES.get_or_init(|| RwLock::new(load_from_env().unwrap_or_default()))
}

/// A usable display name: bounded, printable, no control characters.
pub fn usable_name(value: &str) -> bool {
    let count = value.chars().count();
    (1..=MAX_NAME_CHARACTERS).contains(&count)
        && !value.trim().is_empty()
        && !value.chars().any(char::is_control)
}

fn parse(bytes: &[u8]) -> Option<HashMap<String, String>> {
    let file: File = serde_json::from_slice(bytes).ok()?;
    Some(
        file.members
            .into_iter()
            .filter(|entry| {
                crate::storage::valid_identifier(&entry.principal_id)
                    && usable_name(&entry.display_name)
            })
            .map(|entry| (entry.principal_id, entry.display_name.trim().to_owned()))
            .collect(),
    )
}

fn load_from_env() -> Option<HashMap<String, String>> {
    let path = std::env::var_os(FILE_ENV)?;
    let metadata = std::fs::metadata(&path).ok()?;
    if metadata.len() > MAX_FILE_BYTES {
        tracing::warn!(
            event = "directory.refused",
            code = "too_large",
            "display names file refused"
        );
        return None;
    }
    let bytes = std::fs::read(&path).ok()?;
    let parsed = parse(&bytes);
    tracing::info!(
        event = "directory.loaded",
        count = parsed.as_ref().map(HashMap::len).unwrap_or(0),
        "display names loaded"
    );
    parsed
}

/// The display name the operator set for `principal`, if any.
pub fn display_name(principal: &str) -> Option<String> {
    names().read().ok()?.get(principal).cloned()
}

/// The label people see: the display name, else the neutral
/// `Member <8 hex>` handle (`storage::sharing::member_label`'s rule).
pub fn label(principal: &str) -> String {
    display_name(principal)
        .unwrap_or_else(|| format!("Member {}", &principal[..8.min(principal.len())]))
}

/// Every (principal, name) known; for approver matching.
pub fn all() -> Vec<(String, String)> {
    names()
        .read()
        .map(|map| map.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default()
}

/// Tests and campaigns: replace the directory without a file.
pub fn set_for_tests(entries: &[(&str, &str)]) {
    if let Ok(mut map) = names().write() {
        map.clear();
        for (principal, name) in entries {
            if crate::storage::valid_identifier(principal) && usable_name(name) {
                map.insert(principal.to_string(), name.trim().to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_is_closed_and_names_are_bounded() {
        let parsed = parse(br#"{"members":[{"principal_id":"00000000-0000-4000-8000-00000000006d","display_name":" Sarah Chen "},{"principal_id":"bad","display_name":"x"},{"principal_id":"00000000-0000-4000-8000-000000000066","display_name":"a\nb"}]}"#).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed["00000000-0000-4000-8000-00000000006d"], "Sarah Chen");
        assert!(parse(b"[]").is_none());
        assert!(!usable_name(""));
        assert!(!usable_name(&"n".repeat(81)));
        assert!(usable_name("Priya N."));
    }
}
