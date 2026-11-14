//! Member display names (ADR-011 scenarios, APP-095 debt item 3: principals
//! carry no display name in the Phase 1 schema, so the members list showed
//! "Member <8 hex>" only).
//!
//! The minimal honest form for the pitch: an operator-maintained file named by
//! `GENUI_DISPLAY_NAMES_FILE`, JSON `{"members":[{"principal_id":"<uuid>",
//! "display_name":"Sarah Chen"}, …]}`, read once at startup, with
//! `GENUI_DEMO_MODE` on. A name shows only inside a listed demo organization
//! (`demo::is_demo_org`): fixtures and the demo organization get synthetic
//! names from their campaign, and every other organization sees the neutral
//! `Member <8 hex>`, even for a principal the file lists. A name is a label for
//! people (the members list, the approver chip, the records list), never an
//! identity: authority still comes from the principal id and its grants.
//! Since migration 0023 (trial plan D6) members name themselves:
//! `memberships.display_name`, set through `Workspace::set_display_name` and
//! shown for **any** organization. Because `label` is synchronous and called
//! from many render paths, the stored names live in a process-wide cache
//! keyed by (organization, principal): filled once at startup from one read
//! of the names (`Workspace::load_display_names`), and refreshed by this
//! process's own writes (`set_display_name`) and on admission. The beta runs
//! one task (ADR-007 D7.18, DS-16), so the cache is the database's current
//! state; a second task would need a shared refresh, which is recorded here
//! as the limit. The DS-44 persona file remains the demo-only fill-in.
use serde::Deserialize;
use std::{
    collections::HashMap,
    sync::{OnceLock, RwLock},
};

// DS-44: display names from an operator file or JSON; loaded only with GENUI_DEMO_MODE on (2026-11-12) and shown only in a listed demo organization (2026-11-13, APP-098).
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
/// 0023 member display names, keyed by (organization, principal).
static MEMBER_NAMES: OnceLock<RwLock<HashMap<(String, String), String>>> = OnceLock::new();

fn names() -> &'static RwLock<HashMap<String, String>> {
    NAMES.get_or_init(|| RwLock::new(load_from_env().unwrap_or_default()))
}
fn member_names() -> &'static RwLock<HashMap<(String, String), String>> {
    MEMBER_NAMES.get_or_init(RwLock::default)
}

/// The display name a member set for itself in `organization` (0023), as
/// this process last read or wrote it.
pub fn member_name(organization: &str, principal: &str) -> Option<String> {
    member_names()
        .read()
        .ok()?
        .get(&(organization.to_owned(), principal.to_owned()))
        .cloned()
}

/// Records one member's name as the database now holds it (`None` clears):
/// after this process's own `set_display_name`, and on admission.
pub fn set_member_name(organization: &str, principal: &str, name: Option<&str>) {
    if let Ok(mut map) = member_names().write() {
        let key = (organization.to_owned(), principal.to_owned());
        match name.filter(|n| usable_name(n)) {
            Some(name) => {
                map.insert(key, name.trim().to_owned());
            }
            None => {
                map.remove(&key);
            }
        }
    }
}

/// Replaces every cached name of `organization` with the database's
/// (the startup fill, one organization at a time).
pub fn replace_member_names(organization: &str, entries: &[(String, String)]) {
    if let Ok(mut map) = member_names().write() {
        map.retain(|(org, _), _| org != organization);
        for (principal, name) in entries {
            if crate::storage::valid_identifier(principal) && usable_name(name) {
                map.insert(
                    (organization.to_owned(), principal.clone()),
                    name.trim().to_owned(),
                );
            }
        }
    }
}

/// A usable display name: bounded, printable, no control characters (Cc)
/// and no Unicode format characters (Cf: zero-width, bidi controls and the
/// like), so one name cannot pass for another in a list (trial review m1).
pub fn usable_name(value: &str) -> bool {
    let count = value.chars().count();
    (1..=MAX_NAME_CHARACTERS).contains(&count)
        && !value.trim().is_empty()
        && !value.chars().any(|c| c.is_control() || format_character(c))
}

/// Unicode general category Cf (format characters), Unicode 15.1.
fn format_character(c: char) -> bool {
    const CF: &[(u32, u32)] = &[
        (0x00AD, 0x00AD),
        (0x0600, 0x0605),
        (0x061C, 0x061C),
        (0x06DD, 0x06DD),
        (0x070F, 0x070F),
        (0x0890, 0x0891),
        (0x08E2, 0x08E2),
        (0x180E, 0x180E),
        (0x200B, 0x200F),
        (0x202A, 0x202E),
        (0x2060, 0x2064),
        (0x2066, 0x206F),
        (0xFEFF, 0xFEFF),
        (0xFFF9, 0xFFFB),
        (0x110BD, 0x110BD),
        (0x110CD, 0x110CD),
        (0x13430, 0x1343F),
        (0x1BCA0, 0x1BCA3),
        (0x1D173, 0x1D17A),
        (0xE0001, 0xE0001),
        (0xE0020, 0xE007F),
    ];
    let code = u32::from(c);
    CF.iter().any(|(low, high)| (*low..=*high).contains(&code))
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

/// The display name of `principal` as seen in `organization`, if any: the
/// name the member set (0023, any organization), else the operator's DS-44
/// persona (listed demo organizations only).
pub fn display_name(organization: &str, principal: &str) -> Option<String> {
    if let Some(name) = member_name(organization, principal) {
        return Some(name);
    }
    // DS-44: persona names belong to the listed demo organizations only.
    if !crate::demo::is_demo_org(organization) {
        return None;
    }
    names().read().ok()?.get(principal).cloned()
}

/// The label people in `organization` see: the display name, else the
/// neutral `Member <8 hex>` handle (`storage::sharing::member_label`'s rule).
pub fn label(organization: &str, principal: &str) -> String {
    display_name(organization, principal).unwrap_or_else(|| neutral_label(principal))
}

/// The neutral `Member <8 hex>` handle, never a name.
pub fn neutral_label(principal: &str) -> String {
    format!("Member {}", &principal[..8.min(principal.len())])
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

    /// Trial review m1: no name passes for another through invisible or
    /// direction-changing characters (Unicode Cf), wherever they sit.
    #[test]
    fn format_characters_make_a_name_unusable() {
        for invisible in [
            '\u{00AD}',
            '\u{061C}',
            '\u{200B}',
            '\u{200D}',
            '\u{200E}',
            '\u{202E}',
            '\u{2066}',
            '\u{2069}',
            '\u{FEFF}',
            '\u{E0041}',
        ] {
            assert!(
                !usable_name(&format!("Sarah{invisible}Chen")),
                "{invisible:?}"
            );
            assert!(!usable_name(&format!("{invisible}Sarah")), "{invisible:?}");
            assert!(!usable_name(&invisible.to_string()), "{invisible:?}");
        }
        // Ordinary letters, accents, scripts and emoji stay usable.
        for name in ["Ana María", "Zoë O'Brien", "李小龍", "محمد", "Priya 🌱"] {
            assert!(usable_name(name), "{name}");
        }
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

    /// APP-098 DS-44 (box 5 review M3): a persona name is shown only inside a
    /// listed demo organization. Any other organization sees the neutral
    /// label, even for a principal the names file lists, so a file that named
    /// a real member could not put a persona name on them.
    #[test]
    fn display_names_apply_only_inside_a_listed_demo_organization() {
        let _guard = crate::demo::lock_for_tests();
        const DEMO: &str = "00000000-0000-4000-8000-0000000000d1";
        const OTHER: &str = "00000000-0000-4000-8000-0000000000d2";
        const SARAH: &str = "00000000-0000-4000-8000-00000000006d";
        set_for_tests(&[(SARAH, "Sarah Chen")]);
        crate::demo::configure_for_tests(true, &[DEMO]);
        assert_eq!(display_name(DEMO, SARAH).as_deref(), Some("Sarah Chen"));
        assert_eq!(label(DEMO, SARAH), "Sarah Chen");
        assert_eq!(display_name(OTHER, SARAH), None, "another organization");
        assert_eq!(label(OTHER, SARAH), "Member 00000000");
        crate::demo::configure_for_tests(false, &[DEMO]);
        assert_eq!(label(DEMO, SARAH), "Member 00000000", "the switch is off");
        crate::demo::configure_for_tests(false, &[]);
        set_for_tests(&[]);
    }

    /// 0023 (D6): a member's own name shows in its organization whatever the
    /// demo switch says, never in another organization; the fill replaces an
    /// organization's names wholesale, a clear removes one, and an unusable
    /// name is never cached.
    #[test]
    fn member_names_are_per_organization_and_bounded() {
        let _guard = crate::demo::lock_for_tests();
        const ORG: &str = "00000000-0000-4000-8000-0000000000e1";
        const OTHER: &str = "00000000-0000-4000-8000-0000000000e2";
        const ADA: &str = "00000000-0000-4000-8000-0000000000a1";
        const BOB: &str = "00000000-0000-4000-8000-0000000000b1";
        crate::demo::configure_for_tests(false, &[]);
        set_member_name(ORG, ADA, Some(" Ada Lovelace "));
        assert_eq!(label(ORG, ADA), "Ada Lovelace");
        assert_eq!(label(OTHER, ADA), "Member 00000000");
        set_member_name(ORG, ADA, Some("a\nb"));
        assert_eq!(label(ORG, ADA), "Member 00000000", "unusable clears");
        replace_member_names(
            ORG,
            &[
                (ADA.into(), "Ada".into()),
                (BOB.into(), "x".repeat(81)),
                ("bad".into(), "Eve".into()),
            ],
        );
        assert_eq!(label(ORG, ADA), "Ada");
        assert_eq!(label(ORG, BOB), "Member 00000000");
        replace_member_names(OTHER, &[(BOB.into(), "Bob".into())]);
        assert_eq!(
            (label(ORG, ADA), label(OTHER, BOB)),
            ("Ada".into(), "Bob".into())
        );
        replace_member_names(ORG, &[]);
        assert_eq!(label(ORG, ADA), "Member 00000000");
        assert_eq!(
            label(OTHER, BOB),
            "Bob",
            "another organization's fill is untouched"
        );
        set_member_name(OTHER, BOB, None);
        assert_eq!(label(OTHER, BOB), "Member 00000000");
    }
}
