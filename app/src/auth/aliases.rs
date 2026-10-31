//! Readable sign-in aliases (Victor, 2026-10-31): `/signin/northstar` routes
//! exactly as the organization's random handle does. The map is deployment
//! configuration, `GENUI_SIGNIN_ALIASES_JSON`
//! `{"aliases":[{"name":"northstar","handle":"<26-char handle>"}]}`, read once
//! (the stack sets it at synth, APP-096). A name is lowercase `a-z0-9-`,
//! 2..=40 characters, never 26 characters long (so it cannot shadow a random
//! handle), and maps to a handle of the canonical shape; anything else in the
//! file is ignored. An unknown name is the same neutral sign-in page as an
//! unknown handle. The random handle keeps working, so an organization that
//! wants to stay unlisted simply has no alias. Admin-managed aliases in the
//! database are APP-117.
use std::{collections::HashMap, sync::OnceLock};

pub const ENV: &str = "GENUI_SIGNIN_ALIASES_JSON";
const MAX_BYTES: usize = 16 * 1024;

#[derive(serde::Deserialize)]
struct File {
    aliases: Vec<Entry>,
}
#[derive(serde::Deserialize)]
struct Entry {
    name: String,
    handle: String,
}

static ALIASES: OnceLock<HashMap<String, String>> = OnceLock::new();

fn usable_name(name: &str) -> bool {
    (2..=40).contains(&name.len())
        && name.len() != 26
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !name.starts_with('-')
        && !name.ends_with('-')
}

fn parse(text: &str) -> HashMap<String, String> {
    if text.len() > MAX_BYTES {
        return HashMap::new();
    }
    let Ok(file) = serde_json::from_str::<File>(text) else {
        return HashMap::new();
    };
    file.aliases
        .into_iter()
        .filter(|entry| {
            usable_name(&entry.name) && crate::storage::handle_digest(&entry.handle).is_some()
        })
        .map(|entry| (entry.name, entry.handle))
        .collect()
}

fn aliases() -> &'static HashMap<String, String> {
    ALIASES.get_or_init(|| {
        let map = std::env::var(ENV)
            .map(|text| parse(&text))
            .unwrap_or_default();
        if !map.is_empty() {
            tracing::info!(
                event = "signin.aliases_loaded",
                count = map.len(),
                "sign-in aliases loaded"
            );
        }
        map
    })
}

/// Whether `segment` has an alias's shape. Every alias-shaped segment gets
/// the sign-in form, mapped or not, exactly as every handle-shaped segment
/// does (existence is checked only when the form is posted), so a GET never
/// tells which names exist.
pub fn alias_shaped(segment: &str) -> bool {
    usable_name(segment)
}

/// The handle a `/signin/{segment}` path names: the alias's handle, or the
/// segment itself.
pub fn canonical(segment: &str) -> String {
    aliases()
        .get(segment)
        .cloned()
        .unwrap_or_else(|| segment.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HANDLE: &str = "se45m37vabqrfvbh3gbwn22qkm";

    #[test]
    fn names_map_to_canonical_handles_only() {
        let map = parse(&format!(
            r#"{{"aliases":[{{"name":"northstar","handle":"{HANDLE}"}},{{"name":"Bad Name","handle":"{HANDLE}"}},{{"name":"x","handle":"{HANDLE}"}},{{"name":"aaaaaaaaaaaaaaaaaaaaaaaaaa","handle":"{HANDLE}"}},{{"name":"acme","handle":"not-a-handle"}},{{"name":"-edge","handle":"{HANDLE}"}}]}}"#
        ));
        assert_eq!(map.len(), 1);
        assert_eq!(map.get("northstar").map(String::as_str), Some(HANDLE));
        assert!(parse("not json").is_empty());
        assert!(alias_shaped("acme-corp") && alias_shaped("northstar"));
        assert!(!alias_shaped("Acme") && !alias_shaped(HANDLE) && !alias_shaped("a"));
        assert!(parse(&"x".repeat(MAX_BYTES + 1)).is_empty());
    }
}
