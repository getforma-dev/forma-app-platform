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

/// A usable display name, as the canvas, the admin table, share lists, Ask
/// and Slack show it: 1 to 80 characters with at least one visible base
/// character, and none of these anywhere: control characters (Cc), Unicode
/// format characters (Cf: zero-width, bidi controls and the like) and the
/// line and paragraph separators U+2028 and U+2029. White space, combining
/// marks (Mn, Me) and the blank-looking fillers (`FILLERS`) are allowed
/// inside a name but are not visible base characters, so a name never
/// renders blank and bidi controls cannot reorder the text around it.
///
/// It does not make names unique or tell look-alikes apart: two members may
/// choose the same name. A name is a label, not an identity; the admin table
/// keys each row by principal.
///
/// This is stricter than migration 0023's CHECK on
/// `memberships.display_name`, which can express only the length,
/// `[[:cntrl:]]` and `btrim`: every name accepted here passes it.
pub fn usable_name(value: &str) -> bool {
    let count = value.chars().count();
    (1..=MAX_NAME_CHARACTERS).contains(&count)
        && !value
            .chars()
            .any(|c| c.is_control() || format_character(c) || matches!(c, '\u{2028}' | '\u{2029}'))
        && value.chars().any(visible_base)
}

/// Blank-looking letters and symbols (the Hangul fillers, Braille blank).
const FILLERS: [char; 5] = ['\u{115F}', '\u{1160}', '\u{2800}', '\u{3164}', '\u{FFA0}'];

/// A character that draws something on its own: not white space, not a
/// control or format character, not a combining mark and not a filler.
fn visible_base(c: char) -> bool {
    !c.is_whitespace()
        && !c.is_control()
        && !format_character(c)
        && !combining_mark(c)
        && !FILLERS.contains(&c)
}

/// Unicode general categories Mn and Me (combining marks), Unicode 15.1,
/// generated from Python 3.13's `unicodedata` (343 ranges, sorted).
fn combining_mark(c: char) -> bool {
    let code = u32::from(c);
    MARKS
        .binary_search_by(|&(low, high)| {
            if high < code {
                std::cmp::Ordering::Less
            } else if low > code {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}
#[rustfmt::skip]
const MARKS: &[(u32, u32)] = &[
    (0x0300, 0x036F), (0x0483, 0x0489), (0x0591, 0x05BD), (0x05BF, 0x05BF),
    (0x05C1, 0x05C2), (0x05C4, 0x05C5), (0x05C7, 0x05C7), (0x0610, 0x061A),
    (0x064B, 0x065F), (0x0670, 0x0670), (0x06D6, 0x06DC), (0x06DF, 0x06E4),
    (0x06E7, 0x06E8), (0x06EA, 0x06ED), (0x0711, 0x0711), (0x0730, 0x074A),
    (0x07A6, 0x07B0), (0x07EB, 0x07F3), (0x07FD, 0x07FD), (0x0816, 0x0819),
    (0x081B, 0x0823), (0x0825, 0x0827), (0x0829, 0x082D), (0x0859, 0x085B),
    (0x0898, 0x089F), (0x08CA, 0x08E1), (0x08E3, 0x0902), (0x093A, 0x093A),
    (0x093C, 0x093C), (0x0941, 0x0948), (0x094D, 0x094D), (0x0951, 0x0957),
    (0x0962, 0x0963), (0x0981, 0x0981), (0x09BC, 0x09BC), (0x09C1, 0x09C4),
    (0x09CD, 0x09CD), (0x09E2, 0x09E3), (0x09FE, 0x09FE), (0x0A01, 0x0A02),
    (0x0A3C, 0x0A3C), (0x0A41, 0x0A42), (0x0A47, 0x0A48), (0x0A4B, 0x0A4D),
    (0x0A51, 0x0A51), (0x0A70, 0x0A71), (0x0A75, 0x0A75), (0x0A81, 0x0A82),
    (0x0ABC, 0x0ABC), (0x0AC1, 0x0AC5), (0x0AC7, 0x0AC8), (0x0ACD, 0x0ACD),
    (0x0AE2, 0x0AE3), (0x0AFA, 0x0AFF), (0x0B01, 0x0B01), (0x0B3C, 0x0B3C),
    (0x0B3F, 0x0B3F), (0x0B41, 0x0B44), (0x0B4D, 0x0B4D), (0x0B55, 0x0B56),
    (0x0B62, 0x0B63), (0x0B82, 0x0B82), (0x0BC0, 0x0BC0), (0x0BCD, 0x0BCD),
    (0x0C00, 0x0C00), (0x0C04, 0x0C04), (0x0C3C, 0x0C3C), (0x0C3E, 0x0C40),
    (0x0C46, 0x0C48), (0x0C4A, 0x0C4D), (0x0C55, 0x0C56), (0x0C62, 0x0C63),
    (0x0C81, 0x0C81), (0x0CBC, 0x0CBC), (0x0CBF, 0x0CBF), (0x0CC6, 0x0CC6),
    (0x0CCC, 0x0CCD), (0x0CE2, 0x0CE3), (0x0D00, 0x0D01), (0x0D3B, 0x0D3C),
    (0x0D41, 0x0D44), (0x0D4D, 0x0D4D), (0x0D62, 0x0D63), (0x0D81, 0x0D81),
    (0x0DCA, 0x0DCA), (0x0DD2, 0x0DD4), (0x0DD6, 0x0DD6), (0x0E31, 0x0E31),
    (0x0E34, 0x0E3A), (0x0E47, 0x0E4E), (0x0EB1, 0x0EB1), (0x0EB4, 0x0EBC),
    (0x0EC8, 0x0ECE), (0x0F18, 0x0F19), (0x0F35, 0x0F35), (0x0F37, 0x0F37),
    (0x0F39, 0x0F39), (0x0F71, 0x0F7E), (0x0F80, 0x0F84), (0x0F86, 0x0F87),
    (0x0F8D, 0x0F97), (0x0F99, 0x0FBC), (0x0FC6, 0x0FC6), (0x102D, 0x1030),
    (0x1032, 0x1037), (0x1039, 0x103A), (0x103D, 0x103E), (0x1058, 0x1059),
    (0x105E, 0x1060), (0x1071, 0x1074), (0x1082, 0x1082), (0x1085, 0x1086),
    (0x108D, 0x108D), (0x109D, 0x109D), (0x135D, 0x135F), (0x1712, 0x1714),
    (0x1732, 0x1733), (0x1752, 0x1753), (0x1772, 0x1773), (0x17B4, 0x17B5),
    (0x17B7, 0x17BD), (0x17C6, 0x17C6), (0x17C9, 0x17D3), (0x17DD, 0x17DD),
    (0x180B, 0x180D), (0x180F, 0x180F), (0x1885, 0x1886), (0x18A9, 0x18A9),
    (0x1920, 0x1922), (0x1927, 0x1928), (0x1932, 0x1932), (0x1939, 0x193B),
    (0x1A17, 0x1A18), (0x1A1B, 0x1A1B), (0x1A56, 0x1A56), (0x1A58, 0x1A5E),
    (0x1A60, 0x1A60), (0x1A62, 0x1A62), (0x1A65, 0x1A6C), (0x1A73, 0x1A7C),
    (0x1A7F, 0x1A7F), (0x1AB0, 0x1ACE), (0x1B00, 0x1B03), (0x1B34, 0x1B34),
    (0x1B36, 0x1B3A), (0x1B3C, 0x1B3C), (0x1B42, 0x1B42), (0x1B6B, 0x1B73),
    (0x1B80, 0x1B81), (0x1BA2, 0x1BA5), (0x1BA8, 0x1BA9), (0x1BAB, 0x1BAD),
    (0x1BE6, 0x1BE6), (0x1BE8, 0x1BE9), (0x1BED, 0x1BED), (0x1BEF, 0x1BF1),
    (0x1C2C, 0x1C33), (0x1C36, 0x1C37), (0x1CD0, 0x1CD2), (0x1CD4, 0x1CE0),
    (0x1CE2, 0x1CE8), (0x1CED, 0x1CED), (0x1CF4, 0x1CF4), (0x1CF8, 0x1CF9),
    (0x1DC0, 0x1DFF), (0x20D0, 0x20F0), (0x2CEF, 0x2CF1), (0x2D7F, 0x2D7F),
    (0x2DE0, 0x2DFF), (0x302A, 0x302D), (0x3099, 0x309A), (0xA66F, 0xA672),
    (0xA674, 0xA67D), (0xA69E, 0xA69F), (0xA6F0, 0xA6F1), (0xA802, 0xA802),
    (0xA806, 0xA806), (0xA80B, 0xA80B), (0xA825, 0xA826), (0xA82C, 0xA82C),
    (0xA8C4, 0xA8C5), (0xA8E0, 0xA8F1), (0xA8FF, 0xA8FF), (0xA926, 0xA92D),
    (0xA947, 0xA951), (0xA980, 0xA982), (0xA9B3, 0xA9B3), (0xA9B6, 0xA9B9),
    (0xA9BC, 0xA9BD), (0xA9E5, 0xA9E5), (0xAA29, 0xAA2E), (0xAA31, 0xAA32),
    (0xAA35, 0xAA36), (0xAA43, 0xAA43), (0xAA4C, 0xAA4C), (0xAA7C, 0xAA7C),
    (0xAAB0, 0xAAB0), (0xAAB2, 0xAAB4), (0xAAB7, 0xAAB8), (0xAABE, 0xAABF),
    (0xAAC1, 0xAAC1), (0xAAEC, 0xAAED), (0xAAF6, 0xAAF6), (0xABE5, 0xABE5),
    (0xABE8, 0xABE8), (0xABED, 0xABED), (0xFB1E, 0xFB1E), (0xFE00, 0xFE0F),
    (0xFE20, 0xFE2F), (0x101FD, 0x101FD), (0x102E0, 0x102E0), (0x10376, 0x1037A),
    (0x10A01, 0x10A03), (0x10A05, 0x10A06), (0x10A0C, 0x10A0F), (0x10A38, 0x10A3A),
    (0x10A3F, 0x10A3F), (0x10AE5, 0x10AE6), (0x10D24, 0x10D27), (0x10EAB, 0x10EAC),
    (0x10EFD, 0x10EFF), (0x10F46, 0x10F50), (0x10F82, 0x10F85), (0x11001, 0x11001),
    (0x11038, 0x11046), (0x11070, 0x11070), (0x11073, 0x11074), (0x1107F, 0x11081),
    (0x110B3, 0x110B6), (0x110B9, 0x110BA), (0x110C2, 0x110C2), (0x11100, 0x11102),
    (0x11127, 0x1112B), (0x1112D, 0x11134), (0x11173, 0x11173), (0x11180, 0x11181),
    (0x111B6, 0x111BE), (0x111C9, 0x111CC), (0x111CF, 0x111CF), (0x1122F, 0x11231),
    (0x11234, 0x11234), (0x11236, 0x11237), (0x1123E, 0x1123E), (0x11241, 0x11241),
    (0x112DF, 0x112DF), (0x112E3, 0x112EA), (0x11300, 0x11301), (0x1133B, 0x1133C),
    (0x11340, 0x11340), (0x11366, 0x1136C), (0x11370, 0x11374), (0x11438, 0x1143F),
    (0x11442, 0x11444), (0x11446, 0x11446), (0x1145E, 0x1145E), (0x114B3, 0x114B8),
    (0x114BA, 0x114BA), (0x114BF, 0x114C0), (0x114C2, 0x114C3), (0x115B2, 0x115B5),
    (0x115BC, 0x115BD), (0x115BF, 0x115C0), (0x115DC, 0x115DD), (0x11633, 0x1163A),
    (0x1163D, 0x1163D), (0x1163F, 0x11640), (0x116AB, 0x116AB), (0x116AD, 0x116AD),
    (0x116B0, 0x116B5), (0x116B7, 0x116B7), (0x1171D, 0x1171F), (0x11722, 0x11725),
    (0x11727, 0x1172B), (0x1182F, 0x11837), (0x11839, 0x1183A), (0x1193B, 0x1193C),
    (0x1193E, 0x1193E), (0x11943, 0x11943), (0x119D4, 0x119D7), (0x119DA, 0x119DB),
    (0x119E0, 0x119E0), (0x11A01, 0x11A0A), (0x11A33, 0x11A38), (0x11A3B, 0x11A3E),
    (0x11A47, 0x11A47), (0x11A51, 0x11A56), (0x11A59, 0x11A5B), (0x11A8A, 0x11A96),
    (0x11A98, 0x11A99), (0x11C30, 0x11C36), (0x11C38, 0x11C3D), (0x11C3F, 0x11C3F),
    (0x11C92, 0x11CA7), (0x11CAA, 0x11CB0), (0x11CB2, 0x11CB3), (0x11CB5, 0x11CB6),
    (0x11D31, 0x11D36), (0x11D3A, 0x11D3A), (0x11D3C, 0x11D3D), (0x11D3F, 0x11D45),
    (0x11D47, 0x11D47), (0x11D90, 0x11D91), (0x11D95, 0x11D95), (0x11D97, 0x11D97),
    (0x11EF3, 0x11EF4), (0x11F00, 0x11F01), (0x11F36, 0x11F3A), (0x11F40, 0x11F40),
    (0x11F42, 0x11F42), (0x13440, 0x13440), (0x13447, 0x13455), (0x16AF0, 0x16AF4),
    (0x16B30, 0x16B36), (0x16F4F, 0x16F4F), (0x16F8F, 0x16F92), (0x16FE4, 0x16FE4),
    (0x1BC9D, 0x1BC9E), (0x1CF00, 0x1CF2D), (0x1CF30, 0x1CF46), (0x1D167, 0x1D169),
    (0x1D17B, 0x1D182), (0x1D185, 0x1D18B), (0x1D1AA, 0x1D1AD), (0x1D242, 0x1D244),
    (0x1DA00, 0x1DA36), (0x1DA3B, 0x1DA6C), (0x1DA75, 0x1DA75), (0x1DA84, 0x1DA84),
    (0x1DA9B, 0x1DA9F), (0x1DAA1, 0x1DAAF), (0x1E000, 0x1E006), (0x1E008, 0x1E018),
    (0x1E01B, 0x1E021), (0x1E023, 0x1E024), (0x1E026, 0x1E02A), (0x1E08F, 0x1E08F),
    (0x1E130, 0x1E136), (0x1E2AE, 0x1E2AE), (0x1E2EC, 0x1E2EF), (0x1E4EC, 0x1E4EF),
    (0x1E8D0, 0x1E8D6), (0x1E944, 0x1E94A), (0xE0100, 0xE01EF),
];

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

    /// Recheck N2: a name shows at least one visible base character; one
    /// made only of fillers, blanks or combining marks, or holding a line or
    /// paragraph separator anywhere, is refused.
    #[test]
    fn a_name_needs_a_visible_base_character() {
        for blank in [
            "\u{3164}",
            "\u{FFA0}",
            "\u{115F}",
            "\u{1160}",
            "\u{2800}",
            "\u{3164} \u{3164}",
            "\u{0301}",
            "\u{0301}\u{0302}",
            "\u{20DD}",
            "\u{FE0F}",
            "\u{E0100}",
            "\u{00A0}",
            "\u{3000}",
        ] {
            assert!(!usable_name(blank), "{blank:?}");
        }
        for separated in ["Ana\u{2028}Lopez", "Ana\u{2029}", "\u{2028}Ana"] {
            assert!(!usable_name(separated), "{separated:?}");
        }
        // A mark on a base letter, or a filler beside a letter, is a name.
        for name in ["e\u{0301}", "Ana\u{3164}", "🌱", "1", "Ü"] {
            assert!(usable_name(name), "{name:?}");
        }
        // The table is sorted and disjoint, so the binary search is sound.
        assert!(MARKS.windows(2).all(|w| w[0].1 < w[1].0));
        assert!(MARKS.iter().all(|(low, high)| low <= high));
        assert!(combining_mark('\u{0300}') && combining_mark('\u{036F}'));
        assert!(combining_mark('\u{E01EF}') && !combining_mark('a'));
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
