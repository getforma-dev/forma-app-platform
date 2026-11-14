use super::qualification::PrivateDirectory;
use super::*;
use serde_json::json;
use std::{
    fs,
    os::unix::fs::{symlink, PermissionsExt},
};

#[test]
fn unknown_permissions_and_noncanonical_ids_deny() {
    assert_eq!(Action::try_from("owner"), Err(Error::Denied));
    assert_eq!(Action::try_from("record_read"), Ok(Action::RecordRead));
    assert!(valid_id("00000000-0000-4000-8000-000000000001"));
    assert!(!valid_id("00000000-0000-4000-8000-00000000000A"));
    assert!(!valid_id("00000000-0000-0000-0000-000000000001"));
}

#[test]
fn private_registry_rejects_links_modes_duplicates_unknowns_and_large_documents() {
    let directory = PrivateDirectory::new();
    let file = directory.0.join("registry.json");
    let document = json!({"schema_version":1,"registry_version":"00000000-0000-4000-8000-000000000aaa","bindings":[{"issuer":"fixture-only","provider_tenant_id":"ten_example","organization_id":"00000000-0000-4000-8000-000000000001"}]});
    let write = |value: &serde_json::Value| {
        if file.exists() {
            fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
        }
        fs::write(&file, serde_json::to_vec(value).unwrap()).unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
    };
    write(&document);
    let first = registry::Registry::load(&file).unwrap();
    assert_eq!(first.identity.sha256.len(), 64);
    assert!(first.agrees(
        "fixture-only",
        "ten_example",
        "00000000-0000-4000-8000-000000000001"
    ));
    assert!(!first.agrees(
        "fixture-only",
        "ten_example",
        "00000000-0000-4000-8000-000000000002"
    ));
    for mode in [0o644, 0o666, 0o400] {
        fs::set_permissions(&file, fs::Permissions::from_mode(mode)).unwrap();
        assert!(registry::Registry::load(&file).is_err());
    }
    write(&document);
    fs::set_permissions(&directory.0, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(registry::Registry::load(&file).is_err());
    fs::set_permissions(&directory.0, fs::Permissions::from_mode(0o700)).unwrap();
    let link = directory.0.join("linked");
    symlink(&file, &link).unwrap();
    assert!(registry::Registry::load(&link).is_err());
    fs::remove_file(link).unwrap();
    let hard = directory.0.join("hard");
    fs::hard_link(&file, &hard).unwrap();
    assert!(registry::Registry::load(&file).is_err());
    fs::remove_file(hard).unwrap();
    for change in [
        "unknown",
        "duplicate",
        "conflicting",
        "oversize_bindings",
        "invalid_uuid",
        "invalid_label",
    ] {
        let mut bad = document.clone();
        match change {
            "unknown" => {
                bad["enabled"] = json!(true);
            }
            "duplicate" => {
                bad["bindings"]
                    .as_array_mut()
                    .unwrap()
                    .push(document["bindings"][0].clone());
            }
            "conflicting" => {
                let mut b = document["bindings"][0].clone();
                b["organization_id"] = json!("00000000-0000-4000-8000-000000000002");
                bad["bindings"].as_array_mut().unwrap().push(b);
            }
            "oversize_bindings" => {
                bad["bindings"] = json!(vec![document["bindings"][0].clone(); 33]);
            }
            "invalid_uuid" => {
                bad["registry_version"] = json!("00000000-0000-0000-0000-000000000aaa");
            }
            _ => {
                bad["bindings"][0]["issuer"] = json!("line\nbreak");
            }
        }
        write(&bad);
        assert!(registry::Registry::load(&file).is_err(), "{change}");
    }
    fs::write(&file, vec![b' '; 16385]).unwrap();
    assert!(registry::Registry::load(&file).is_err());
    write(&document);
    let second = registry::Registry::load(&file).unwrap();
    assert_eq!(first.identity, second.identity);
    let mut empty = document.clone();
    empty["bindings"] = json!([]);
    write(&empty);
    let empty = registry::Registry::load(&file).unwrap();
    assert!(!empty.agrees(
        "fixture-only",
        "ten_example",
        "00000000-0000-4000-8000-000000000001"
    ));
}

#[test]
fn production_settings_allow_only_fixed_rds_endpoint() {
    for host in [
        "localhost",
        "127.0.0.1",
        "rds.example.com",
        "db.us-east-1.rds.amazonaws.com.evil",
        "DB.us-east-1.rds.amazonaws.com",
    ] {
        assert!(Settings::new("c".into(), "r".into(), "ca".into(), host.into()).is_err());
    }
    assert!(Settings::new(
        "c".into(),
        "r".into(),
        "ca".into(),
        "database.cluster-abc.us-east-1.rds.amazonaws.com".into()
    )
    .is_ok());
}

/// APP-191 (D7): the one row rule. Every non-test statement of `storage`
/// that reads or writes an internal record table carries
/// `authorization::own_rows(`, or is one of the named exceptions: the
/// version and lock reads of `internal_records` (they disclose no value and
/// precede the rule's judgment), `copy_capacity` (a work-wide bound by
/// design), and the inserts and updates of a commit. A statement is the
/// source from the previous `;` to the next one; fragments (`const`s naming
/// the tables) count through every statement that interpolates them. Each
/// exception must match exactly one statement, so the list cannot rot.
#[test]
fn every_record_statement_carries_own_rows() {
    const ALLOWED: [(&str, &str); 7] = [
        ("commands.rs", "let sql = if lock {"),
        (
            "commands.rs",
            "let Some(current) = locked(transaction, \"SELECT current_version::bigint FROM genui.internal_records",
        ),
        ("commands.rs", "INSERT INTO genui.internal_record_revisions("),
        ("commands.rs", "INSERT INTO genui.internal_records("),
        (
            "commands.rs",
            "UPDATE genui.internal_records SET current_version=$6::bigint",
        ),
        (
            "commands.rs",
            "coalesce(max(CASE WHEN r.collection_id=$4::text::uuid AND r.record_id=$5::text::uuid THEN octet_length(v.canonical_values) ELSE 0 END),0)",
        ),
        (
            "actions.rs",
            "UPDATE genui.internal_records SET current_version=current_version WHERE organization_id=$1::text::uuid",
        ),
    ];
    let storage = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/storage");
    let mut files: Vec<_> = fs::read_dir(&storage)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "rs"))
        .filter(|path| {
            let name = path.file_name().unwrap().to_str().unwrap();
            !(name.ends_with("_campaign.rs")
                || name.ends_with("_qualification.rs")
                || name == "qualification_cases.rs"
                || name == "qualification.rs"
                || name == "tests.rs"
                || name == "work_presentation_checks.rs")
        })
        .collect();
    files.sort();
    let mut with_rule = 0usize;
    let mut allowed_hits = vec![0usize; ALLOWED.len()];
    let mut unguarded = Vec::new();
    for path in &files {
        let name = path.file_name().unwrap().to_str().unwrap().to_owned();
        let raw = fs::read_to_string(path).unwrap();
        // Non-test source only, without comments.
        let code = raw
            .split("#[cfg(test)]\nmod tests")
            .next()
            .unwrap()
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut needles = vec!["internal_record".to_owned()];
        for line in code.lines() {
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed
                .strip_prefix("pub(super) const ")
                .or_else(|| trimmed.strip_prefix("pub(crate) const "))
                .or_else(|| trimmed.strip_prefix("pub const "))
                .or_else(|| trimmed.strip_prefix("const "))
            {
                if line.contains("internal_record") {
                    let name = rest.split(':').next().unwrap().trim();
                    needles.push(format!("{{{name}}}"));
                }
            }
        }
        let mut seen = std::collections::BTreeSet::new();
        for needle in &needles {
            for (at, _) in code.match_indices(needle.as_str()) {
                let start = code[..at].rfind(';').map_or(0, |i| i + 1);
                let end = code[at..].find(';').map_or(code.len(), |i| at + i + 1);
                if !seen.insert(start) {
                    continue;
                }
                let statement = &code[start..end];
                let head = statement.trim_start();
                if head.starts_with("const ") || head.starts_with("pub(super) const ") {
                    continue; // a fragment, judged through its uses
                }
                if statement.contains("own_rows(") {
                    with_rule += 1;
                    continue;
                }
                let hit = ALLOWED
                    .iter()
                    .position(|(file, text)| *file == name && statement.contains(text));
                match hit {
                    Some(index) => allowed_hits[index] += 1,
                    None => unguarded.push(format!("{name}: {}", statement.trim())),
                }
            }
        }
    }
    assert!(
        unguarded.is_empty(),
        "statements naming an internal record table without own_rows( and not on the allowlist:\n{}",
        unguarded.join("\n---\n")
    );
    for (index, hits) in allowed_hits.iter().enumerate() {
        assert_eq!(
            *hits, 1,
            "allowlist entry {:?} matched {hits} statements",
            ALLOWED[index]
        );
    }
    // The scan found the rule where it is expected (a broken scanner cannot
    // pass vacuously): the listings (batch, list, search, target), the query
    // snapshot and its release, the record components (bounds and rows),
    // `Permit::record`, `current_state`, the receipts, the source-view
    // sequences (member and guest) and the event judgment.
    assert!(with_rule >= 14, "own_rows statements found: {with_rule}");
}
