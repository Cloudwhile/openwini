use anyhow::Result;
use serde_json::{json, Value};

use crate::github;
use crate::state::AppState;
use crate::validation as v;

/// 仓库三项权属校验：公开仓库 / owner 或 admin 权限 / Release 存在 ZIP 附件
/// errors 为中文兜底文案；errorCodes 与 errors 一一对应，供前端按语言本地化
pub async fn validate_repository(state: &AppState, token: &str, repo_url: &str) -> Result<Value> {
    let (owner, repo) = v::parse_repo_url(repo_url)
        .map_err(|e| crate::github::coded("repo_url_invalid", serde_json::json!({}), e))?;
    let gh_repo = github::get_repo(state, token, &owner, &repo).await?;
    let mut errors: Vec<String> = Vec::new();
    let mut error_codes: Vec<&str> = Vec::new();
    if gh_repo.private {
        errors.push("仓库不是公开仓库（私有仓库的插件不被接受）".into());
        error_codes.push("repo_private");
    }
    let has_admin = gh_repo
        .permissions
        .as_ref()
        .map(|p| p.admin)
        .unwrap_or(false);
    if !has_admin {
        errors.push("你的 GitHub 账号对该仓库没有 owner 或 admin 权限".into());
        error_codes.push("repo_no_admin");
    }
    let releases = if gh_repo.private || !has_admin {
        Vec::new()
    } else {
        github::list_releases(state, token, &owner, &repo)
            .await
            .unwrap_or_default()
    };
    let has_zip = releases.iter().any(|r| {
        !r.draft
            && r.assets
                .iter()
                .any(|a| a.name.to_ascii_lowercase().ends_with(".zip"))
    });
    if !has_zip {
        errors.push("仓库 Release 中不存在插件 ZIP 附件".into());
        error_codes.push("release_no_zip");
    }
    Ok(json!({
        "owner": owner,
        "repo": repo,
        "private": gh_repo.private,
        "hasAdmin": has_admin,
        "hasReleaseZip": has_zip,
        "ok": errors.is_empty(),
        "errors": errors,
        "errorCodes": error_codes,
        "defaultBranch": gh_repo.default_branch,
    }))
}

/// 清单 + Release 预览：供提交表单展示（清单自动比对结果）
pub async fn manifest_preview(state: &AppState, token: &str, repo_url: &str) -> Result<Value> {
    let (owner, repo) = v::parse_repo_url(repo_url).map_err(|e| anyhow::anyhow!("{e}"))?;
    let gh_repo = github::get_repo(state, token, &owner, &repo).await?;
    if gh_repo.private {
        anyhow::bail!("仓库不是公开仓库");
    }
    let branch = gh_repo
        .default_branch
        .clone()
        .unwrap_or_else(|| "main".into());
    let manifest = github::fetch_manifest(state, token, &owner, &repo, &branch).await?;
    let releases = github::list_releases(state, token, &owner, &repo).await?;
    let release_list: Vec<Value> = releases
        .iter()
        .filter(|r| !r.draft)
        .map(|r| {
            json!({
                "tag": r.tag_name,
                "prerelease": r.prerelease,
                "publishedAt": r.published_at,
                "zipAssets": r.assets.iter()
                    .filter(|a| a.name.to_ascii_lowercase().ends_with(".zip"))
                    .map(|a| json!({"name": a.name, "size": a.size}))
                    .collect::<Vec<_>>(),
            })
        })
        .collect();
    Ok(json!({
        "repositoryUrl": format!("https://github.com/{owner}/{repo}"),
        "defaultBranch": branch,
        "manifest": manifest,
        "releases": release_list,
    }))
}

pub struct ValidatedSubmission {
    pub version: String,
    pub manifest_json: Value,
    pub manifest_name: String,
    pub description: String,
    pub description_en: String,
    pub permissions: Vec<String>,
    pub download_url: String,
    pub sha256: String,
    pub min_host_version: String,
    pub contract_version: i64,
}

#[allow(clippy::too_many_arguments)]
pub async fn validate_full(
    state: &AppState,
    token: &str,
    plugin_id_form: &str,
    repo_url: &str,
    version_form: &str,
    description_form: Option<&str>,
    description_en_form: Option<&str>,
    known_permissions: &std::collections::HashSet<String>,
) -> Result<ValidatedSubmission, Vec<String>> {
    let mut diffs: Vec<String> = Vec::new();

    // —— 形式校验（表单侧）——
    if !v::valid_plugin_id(plugin_id_form) {
        diffs.push(format!(
            "插件标识 {plugin_id_form} 不符合格式 wini_plugin_<name>（name 以小写字母开头，仅含小写字母/数字/下划线，总长 ≤80）"
        ));
    } else if v::is_reserved_plugin_id(plugin_id_form) {
        diffs.push(format!(
            "插件标识 {plugin_id_form} 是内置保留标识，不可使用"
        ));
    }
    if !v::valid_semver(version_form) {
        diffs.push(format!(
            "版本 {version_form} 不是严格 SemVer 2.0（数字段不能有前导零）"
        ));
    }
    if let Some(d) = description_form {
        if d.chars().count() > 8000 {
            diffs.push("描述超过 8000 字符上限".into());
        }
    }
    // 英文描述必填（官方直发传 None，取清单值）
    let description_en_form = match description_en_form {
        Some(d) => {
            let d = d.trim();
            if d.is_empty() {
                diffs.push("英文描述（Description EN）必填".into());
            }
            if d.chars().count() > 8000 {
                diffs.push("英文描述超过 8000 字符上限".into());
            }
            Some(d)
        }
        None => None,
    };

    // —— 仓库权属三项 ——
    let repo_check = validate_repository(state, token, repo_url).await;
    let (owner, repo_name) = match v::parse_repo_url(repo_url) {
        Ok(x) => x,
        Err(e) => {
            diffs.push(e);
            return Err(diffs);
        }
    };
    match repo_check {
        Ok(check) => {
            if let Some(errs) = check.get("errors").and_then(|e| e.as_array()) {
                for e in errs {
                    if let Some(s) = e.as_str() {
                        diffs.push(s.to_string());
                    }
                }
            }
        }
        Err(e) => diffs.push(format!("仓库校验失败：{e}")),
    }
    if !diffs.is_empty() {
        return Err(diffs);
    }

    // —— 清单读取与逐项比对 ——
    let gh_repo = github::get_repo(state, token, &owner, &repo_name)
        .await
        .map_err(|e| vec![format!("读取仓库信息失败：{e}")])?;
    let branch = gh_repo
        .default_branch
        .clone()
        .unwrap_or_else(|| "main".into());
    let manifest = github::fetch_manifest(state, token, &owner, &repo_name, &branch)
        .await
        .map_err(|e| vec![format!("读取仓库根目录 manifest.json 失败：{e}")])?;

    let m_id = manifest.get("id").and_then(|x| x.as_str()).unwrap_or("");
    let m_name = manifest.get("name").and_then(|x| x.as_str()).unwrap_or("");
    let m_ver = manifest
        .get("version")
        .and_then(|x| x.as_str())
        .unwrap_or("");
    let m_desc = manifest.get("description").and_then(|x| x.as_str());
    // 可选的英文清单字段 description_en：存在时必须与表单英文描述一致
    let m_desc_en = manifest.get("description_en").and_then(|x| x.as_str());
    let m_min = manifest
        .get("minHostVersion")
        .and_then(|x| x.as_str())
        .unwrap_or("");
    let m_contract = manifest.get("contractVersion").and_then(|x| x.as_i64());
    let m_perms = match manifest.get("permissions").and_then(|x| x.as_array()) {
        Some(items) if items.iter().all(|item| item.is_string()) => items
            .iter()
            .filter_map(|item| item.as_str().map(String::from))
            .collect::<Vec<_>>(),
        _ => {
            diffs.push("清单 permissions 必须是字符串数组（无权限时填写空数组）".into());
            Vec::new()
        }
    };

    if m_id.is_empty() {
        diffs.push("清单缺少 id 字段".into());
    } else if m_id != plugin_id_form {
        diffs.push(format!(
            "清单 id（{m_id}）与表单插件标识（{plugin_id_form}）不一致"
        ));
    }
    if m_ver.is_empty() {
        diffs.push("清单缺少 version 字段".into());
    } else if m_ver != version_form {
        diffs.push(format!(
            "清单 version（{m_ver}）与表单版本（{version_form}）不一致"
        ));
    }
    match m_desc {
        None => diffs.push("清单缺少 description 字段".into()),
        // 开发者申请：描述必须与表单完全一致；官方直发：以清单为准（None 表示未提供表单值）
        Some(d) => {
            if d.chars().count() > 8000 {
                diffs.push("清单 description 超过 8000 字符上限".into());
            }
            if let Some(form) = description_form {
                if d != form {
                    diffs.push("清单 description 与表单描述不一致（描述必须与清单完全相同）".into())
                }
            }
        }
    }
    if m_desc_en.is_some_and(|description| description.chars().count() > 8000) {
        diffs.push("清单 description_en 超过 8000 字符上限".into());
    }
    if let (Some(m), Some(form)) = (m_desc_en, description_en_form) {
        if m != form {
            diffs.push("清单 description_en 与表单英文描述不一致".into());
        }
    }
    if m_name.is_empty() || m_name.chars().count() > 200 {
        diffs.push("清单 name 必须非空且不超过 200 字符".into());
    }
    if !v::valid_semver(m_min) {
        diffs.push(format!("清单 minHostVersion（{m_min}）不是严格 SemVer 2.0"));
    }
    match m_contract {
        Some(c) if (1..=i64::from(i32::MAX)).contains(&c) => {}
        _ => diffs.push("清单 contractVersion 必须是 1 至 2147483647 的整数".into()),
    }
    if let Err(e) = v::validate_permissions(&m_perms, known_permissions) {
        diffs.push(format!("权限列表问题：{e}"));
    }
    if !diffs.is_empty() {
        return Err(diffs);
    }

    // —— Release 匹配与附件选择 ——
    let releases = github::list_releases(state, token, &owner, &repo_name)
        .await
        .map_err(|e| vec![format!("读取 Release 列表失败：{e}")])?;
    let matched = releases
        .iter()
        .filter(|r| {
            !r.draft && (r.tag_name == version_form || r.tag_name == format!("v{version_form}"))
        })
        .find(|r| {
            r.assets
                .iter()
                .any(|a| a.name.to_ascii_lowercase().ends_with(".zip"))
        });
    let Some(release) = matched else {
        return Err(vec![format!(
            "Release 中未找到与版本 {version_form} 匹配的发布（tag 需为 {version_form} 或 v{version_form}）且包含 ZIP 附件"
        )]);
    };
    let zips: Vec<&github::GhAsset> = release
        .assets
        .iter()
        .filter(|a| a.name.to_ascii_lowercase().ends_with(".zip"))
        .collect();
    let asset = if let Some(a) = zips
        .iter()
        .find(|a| a.name == format!("{plugin_id_form}.zip"))
    {
        a
    } else if zips.len() == 1 {
        zips[0]
    } else {
        return Err(vec![format!(
            "该 Release 包含多个 ZIP 附件（{}），无法确定插件包。请只保留一个 ZIP，或将其命名为 {plugin_id_form}.zip",
            zips.iter().map(|a| a.name.clone()).collect::<Vec<_>>().join("、")
        )]);
    };

    // downloadUrl 一律按 GitHub Release 附件规范地址重建（不做平台中转）
    let download_url = format!(
        "https://github.com/{owner}/{repo_name}/releases/download/{}/{}",
        release.tag_name, asset.name
    );
    if let Err(e) = v::validate_download_url(&download_url, repo_url) {
        return Err(vec![format!("附件地址不符合契约：{e}")]);
    }

    // —— ZIP 附件有效核验：可下载 + 真实 ZIP + 根目录 manifest.json 与申请一致 ——
    if asset.browser_download_url != download_url {
        return Err(vec![
            "Release 附件地址与目录下载地址不一致，请检查附件与版本标签".into(),
        ]);
    }
    let (zip_bytes, sha256) = github::download_and_hash_asset(state, &download_url)
        .await
        .map_err(|e| vec![format!("附件核验失败：{e}")])?;
    let packaged_manifest = crate::zip::verify_plugin_zip(&zip_bytes, plugin_id_form, version_form)
        .map_err(|e| vec![format!("插件包核验未通过：{e}")])?;
    if packaged_manifest != manifest {
        return Err(vec![
            "ZIP 内 manifest.json 与仓库清单不一致，请用同一份清单构建并重新上传 Release 附件"
                .into(),
        ]);
    }

    // 英文描述取值优先级：清单 description_en > 表单填写 > 空
    let description_en_final = m_desc_en
        .map(|s| s.to_string())
        .or_else(|| description_en_form.map(|s| s.to_string()))
        .unwrap_or_default();
    // 注意字段顺序：manifest_json 的移动放在最后，避免与 m_name/m_min 等借用冲突
    Ok(ValidatedSubmission {
        version: version_form.to_string(),
        manifest_name: m_name.to_string(),
        description: m_desc.unwrap_or_default().to_string(),
        description_en: description_en_final,
        permissions: m_perms,
        download_url,
        sha256,
        min_host_version: m_min.to_string(),
        contract_version: m_contract.unwrap_or(1),
        manifest_json: manifest,
    })
}

pub async fn known_permission_set(state: &AppState) -> Result<std::collections::HashSet<String>> {
    let rows: Vec<(String,)> = sqlx::query_as("SELECT name FROM known_permissions")
        .fetch_all(state.pool())
        .await?;
    Ok(rows.into_iter().map(|(n,)| n).collect())
}
