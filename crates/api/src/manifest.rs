use crate::{plugin_security, validation as v};
use serde_json::Value;
use std::collections::HashSet;

pub fn minimum_host_version(manifest: &Value) -> Result<&str, String> {
    // The package/SDK field differs from the directory's minHostVersion field.
    if let Some(legacy) = manifest.get("minHostVersion") {
        if legacy.as_str().is_none() || manifest.get("minimumHostVersion") != Some(legacy) {
            return Err("清单请使用 minimumHostVersion；保留 minHostVersion 时两者必须一致".into());
        }
    }
    match manifest.get("minimumHostVersion") {
        None | Some(Value::Null) => Ok("0.0.0"),
        Some(Value::String(value)) if v::valid_semver(value) => Ok(value),
        _ => Err("清单 minimumHostVersion 必须符合 SemVer".into()),
    }
}

pub fn relative_path(path: &str) -> bool {
    !path.trim().is_empty()
        && !path.chars().any(|c| c < ' ' || ":\\<>\"|?*".contains(c))
        && path
            .split('/')
            .all(|part| !matches!(part, "" | "." | "..") && !part.ends_with(['.', ' ']))
}

/// Validate installable metadata without executing plugin code. Catalog compatibility is checked by the host.
pub fn validate(
    manifest: &Value,
    id: &str,
    version: &str,
    developer: &str,
    repository: &str,
    known: &HashSet<String>,
) -> Result<(), String> {
    let string = |key| manifest.get(key).and_then(Value::as_str).unwrap_or("");
    if string("id") != id || !v::valid_plugin_id(id) || v::is_reserved_plugin_id(id) {
        return Err("清单 id 无效或与插件标识不一致".into());
    }
    if string("version") != version || !v::valid_semver(version) {
        return Err("清单 version 无效或与申请版本不一致".into());
    }
    if string("developerId") != developer
        || !v::valid_developer_id(developer)
        || string("repositoryUrl") != repository
        || v::parse_repo_url(repository).is_err()
    {
        return Err("清单 developerId 或 repositoryUrl 与申请来源不一致".into());
    }
    if string("name").trim().is_empty()
        || string("name").encode_utf16().count() > 200
        || string("entryType").trim().is_empty()
        || !relative_path(string("entryAssembly"))
    {
        return Err("清单名称或入口无效".into());
    }
    for (key, maximum) in [
        ("author", 200),
        ("description", 8000),
        ("description_en", 8000),
    ] {
        if let Some(value) = manifest.get(key).filter(|v| !v.is_null()) {
            if value
                .as_str()
                .is_none_or(|text| text.encode_utf16().count() > maximum)
            {
                return Err(format!("清单 {key} 无效或过长"));
            }
        }
    }
    minimum_host_version(manifest)?;
    if !manifest
        .get("contractVersion")
        .and_then(Value::as_i64)
        .is_some_and(|c| (1..=i64::from(i32::MAX)).contains(&c))
    {
        return Err("清单 contractVersion 必须是 1 至 2147483647 的整数".into());
    }
    let permissions: Vec<String> =
        serde_json::from_value(manifest.get("permissions").cloned().unwrap_or(Value::Null))
            .map_err(|_| "清单 permissions 必须是字符串数组")?;
    v::validate_permissions(&permissions, known)?;
    plugin_security::validate(manifest, &permissions)
}
