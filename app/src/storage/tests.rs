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
