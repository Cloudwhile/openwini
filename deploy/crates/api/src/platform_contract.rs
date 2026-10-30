use crate::{manifest, plugin_security, validation as v};
use serde_json::{json, Value};
use std::collections::HashSet;

fn fixture() -> Value {
    serde_json::from_str(include_str!("../../../tests/fixtures/plugin-contract.json")).unwrap()
}
fn known() -> HashSet<String> {
    v::HOST_PERMISSIONS
        .iter()
        .copied()
        .chain(["custom", "storage", "clipboard"])
        .map(str::to_string)
        .collect()
}

#[test]
fn host_and_platform_accept_the_same_metadata() {
    let f = fixture();
    for (group, check) in [
        ("versions", v::valid_semver as fn(&str) -> bool),
        ("repositories", |s| v::parse_repo_url(s).is_ok()),
        ("downloads", |s| {
            v::validate_download_url(s, "https://github.com/example/plugins").is_ok()
        }),
    ] {
        for case in f[group].as_array().unwrap() {
            assert_eq!(
                check(case["value"].as_str().unwrap()),
                case["valid"].as_bool().unwrap(),
                "{group}: {case}"
            );
        }
    }
    for case in f["permissions"].as_array().unwrap() {
        let permissions: Vec<String> = serde_json::from_value(case["value"].clone()).unwrap();
        assert_eq!(
            v::validate_permissions(&permissions, &known()).is_ok(),
            case["valid"].as_bool().unwrap(),
            "{case}"
        );
    }
    for case in f["networks"].as_array().unwrap() {
        let mut m = f["manifest"].clone();
        m["network"] = case["value"].clone();
        assert_eq!(
            plugin_security::validate(&m, &["network".into()]).is_ok(),
            case["valid"].as_bool().unwrap(),
            "{case}"
        );
    }
    let m = &f["manifest"];
    manifest::validate(
        m,
        "wini_plugin_demo",
        "1.2.3",
        "winiofficial",
        "https://github.com/example/plugins",
        &known(),
    )
    .unwrap();
    let entry: v::CatalogEntry = serde_json::from_value(f["entry"].clone()).unwrap();
    entry.validate_package(m, &known()).unwrap();
    // Export actual platform serialization for the host consumer check in the Win10 guest.
    if let Some(directory) = std::env::var_os("WINI_CONTRACT_OUTPUT") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let source: crate::catalog::SourceEntry =
            serde_json::from_value(f["source"].clone()).unwrap();
        for (name, content) in [
            ("sources.json", json!([source])),
            ("plugins.json", json!([entry])),
            ("manifest.json", m.clone()),
        ] {
            std::fs::write(
                directory.join(name),
                serde_json::to_vec_pretty(&content).unwrap(),
            )
            .unwrap();
        }
    }
}

#[test]
fn package_minimum_and_provenance_cannot_diverge() {
    let f = fixture();
    let valid = |m: &Value| {
        manifest::validate(
            m,
            "wini_plugin_demo",
            "1.2.3",
            "winiofficial",
            "https://github.com/example/plugins",
            &known(),
        )
        .is_ok()
    };
    for (key, value) in [
        ("developerId", json!("other")),
        ("repositoryUrl", json!("https://github.com/other/repo")),
        ("entryAssembly", json!("../Demo.dll")),
        ("entryType", json!(" ")),
        ("name", json!(" ")),
        ("contractVersion", json!(0)),
        ("permissions", json!(["unregistered"])),
        ("minimumHostVersion", json!("1.0.0-01")),
        ("terms", json!({"revision":"1", "text":""})),
    ] {
        let mut m = f["manifest"].clone();
        m[key] = value;
        assert!(!valid(&m), "{key}");
    }
    let mut m = f["manifest"].clone();
    m["minHostVersion"] = m["minimumHostVersion"].clone();
    assert!(valid(&m));
    m["minHostVersion"] = json!("0.2.0");
    assert!(!valid(&m));
    m.as_object_mut().unwrap().remove("minimumHostVersion");
    assert!(!valid(&m));
    m.as_object_mut().unwrap().remove("minHostVersion");
    assert!(valid(&m));
    assert_eq!(manifest::minimum_host_version(&m).unwrap(), "0.0.0");
    let mut m = f["manifest"].clone();
    m["name"] = json!("😀".repeat(101));
    assert!(!valid(&m));
    for id in v::RESERVED_PLUGIN_IDS {
        assert!(v::is_reserved_plugin_id(id));
    }
    assert!(!v::is_reserved_plugin_id("wini_plugin_wechat"));
    assert!(!v::is_reserved_plugin_id("wini_plugin_telegram"));
    let entry: v::CatalogEntry = serde_json::from_value(f["entry"].clone()).unwrap();
    for (key, value) in [
        ("contractVersion", json!(2)),
        ("permissions", json!(["network"])),
        ("minimumHostVersion", json!("0.2.0")),
    ] {
        let mut manifest = f["manifest"].clone();
        manifest[key] = value;
        assert!(
            entry.validate_package(&manifest, &known()).is_err(),
            "{key}"
        );
    }
}
