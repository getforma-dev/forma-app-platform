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

// DS-44: display names from an operator file or JSON; behind GENUI_DEMO_MODE since 2026-11-12 (APP-098).
pub const FILE_ENV: &str = "GENUI_DISPLAY_NAMES_FILE";
/// The same JSON inline (the deployed task: the stack sets it at synth from a
/// private file, APP-096); the file wins when both are set.
pub const JSON_ENV: &str = "GENUI_DISPLAY_NAMES_JSON";
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
    load(
        crate::demo::is_on(),
        std::env::var_os(FILE_ENV),
        std::env::var(JSON_ENV).ok(),
    )
}

/// APP-098 DS-02, DS-44: the names are the demo personas'. The deploy passes
/// them only to windows where `GENUI_DEMO_MODE` is on, and a clean trial
/// refuses them. With the switch off none are loaded, even if set.
fn load(
    switch_on: bool,
    file: Option<std::ffi::OsString>,
    inline: Option<String>,
) -> Option<HashMap<String, String>> {
    if !switch_on {
        if file.is_some() || inline.is_some() {
            tracing::warn!(
                event = "directory.refused",
                code = "demo_mode_off",
                "display names need GENUI_DEMO_MODE"
            );
        }
        return None;
    }
    let Some(path) = file else {
        let inline = inline?;
        if inline.len() as u64 > MAX_FILE_BYTES {
            tracing::warn!(
                event = "directory.refused",
                code = "too_large",
                "display names refused"
            );
            return None;
        }
        let parsed = parse(inline.as_bytes());
        tracing::info!(
            event = "directory.loaded",
            count = parsed.as_ref().map(HashMap::len).unwrap_or(0),
            "display names loaded"
        );
        return parsed;
    };
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

/// Tests and campaigns: replace the directory without a file.
#[cfg(test)]
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

    /// APP-098 DS-02, DS-44: with the demo switch off no display name is
    /// loaded, from the inline JSON or the file; with it on, they are.
    #[test]
    fn display_names_load_only_with_the_demo_switch_on() {
        let json = r#"{"members":[{"principal_id":"00000000-0000-4000-8000-00000000006d","display_name":"Sarah Chen"}]}"#;
        assert_eq!(load(false, None, Some(json.into())), None);
        let dir = std::env::temp_dir().join(format!("forma-directory-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("names.json");
        std::fs::write(&file, json).unwrap();
        assert_eq!(load(false, Some(file.clone().into()), None), None);
        let on = load(true, Some(file.into()), None).unwrap();
        assert_eq!(on["00000000-0000-4000-8000-00000000006d"], "Sarah Chen");
        assert_eq!(load(true, None, Some(json.into())).unwrap().len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
