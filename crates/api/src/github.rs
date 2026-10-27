use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::state::AppState;
use crate::validation::{MANIFEST_MAX_BYTES, PLUGIN_ZIP_MAX_BYTES};

pub const OAUTH_SCOPES: &str = "read:user user:email public_repo";
const GITHUB_API_MAX_BYTES: usize = 8 * 1024 * 1024;
const OAUTH_MAX_BYTES: usize = 64 * 1024;

async fn read_bounded_response(resp: reqwest::Response, limit: usize) -> Result<Vec<u8>> {
    if resp
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        anyhow::bail!("GitHub 响应超过大小上限");
    }
    let mut bytes = Vec::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("读取 GitHub 响应中断")?;
        if chunk.len() > limit.saturating_sub(bytes.len()) {
            anyhow::bail!("GitHub 响应超过大小上限");
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

#[derive(Debug, Clone, Deserialize)]
pub struct GhUser {
    pub id: i64,
    pub login: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GhEmail {
    pub email: String,
    pub primary: bool,
    pub verified: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GhRepo {
    pub full_name: String,
    pub html_url: String,
    pub private: bool,
    pub description: Option<String>,
    pub default_branch: Option<String>,
    pub permissions: Option<GhRepoPerms>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GhRepoPerms {
    pub admin: bool,
    #[serde(default)]
    pub push: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GhRelease {
    pub tag_name: String,
    pub draft: bool,
    pub prerelease: bool,
    pub published_at: Option<String>,
    pub assets: Vec<GhAsset>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GhAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GhContent {
    pub name: String,
    pub content: Option<String>,
    pub encoding: Option<String>,
}

pub async fn exchange_code(state: &AppState, code: &str) -> Result<String> {
    let cfg = state.config();
    let resp = state
        .http()
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", cfg.github_client_id.as_str()),
            ("client_secret", cfg.github_client_secret.as_str()),
            ("code", code),
        ])
        .send()
        .await
        .context("请求 GitHub OAuth token 失败")?;
    let status = resp.status();
    let bytes = read_bounded_response(resp, OAUTH_MAX_BYTES).await?;
    if !status.is_success() {
        anyhow::bail!("GitHub OAuth 请求失败（{status}）");
    }
    let json: serde_json::Value =
        serde_json::from_slice(&bytes).context("解析 GitHub OAuth 响应失败")?;
    if let Some(err) = json.get("error").and_then(|v| v.as_str()) {
        anyhow::bail!("GitHub OAuth 失败: {err}");
    }
    json.get("access_token")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("GitHub OAuth 响应缺少 access_token"))
}

async fn gh_get<T: for<'de> Deserialize<'de>>(
    state: &AppState,
    token: &str,
    path: &str,
) -> Result<T> {
    let url = format!("https://api.github.com{path}");
    let resp = state
        .http()
        .get(&url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .with_context(|| format!("请求 GitHub API 失败（网络错误）: {path}"))?;
    let status = resp.status();
    let bytes = read_bounded_response(resp, GITHUB_API_MAX_BYTES).await?;
    let text = String::from_utf8_lossy(&bytes);
    if !status.is_success() {
        // 404 按资源类型给出可操作的中文提示；其余状态码附简短片段，不带 token
        if status == reqwest::StatusCode::NOT_FOUND {
            if path.contains("/contents/") {
                anyhow::bail!("NOT_FOUND_FILE");
            }
            if path.starts_with("/repos/") {
                anyhow::bail!("NOT_FOUND_REPO");
            }
            anyhow::bail!("GitHub 返回 404：资源不存在（{path}）");
        }
        if status == reqwest::StatusCode::UNAUTHORIZED {
            anyhow::bail!("GitHub 访问令牌无效或已过期（401），请断开后重新连接 GitHub 授权");
        }
        if status == reqwest::StatusCode::FORBIDDEN {
            anyhow::bail!(
                "GitHub 拒绝访问（403，可能是 API 速率限制）：{}",
                truncate(&text, 120)
            );
        }
        anyhow::bail!("GitHub API 请求失败（{status}）：{}", truncate(&text, 200));
    }
    serde_json::from_slice(&bytes).with_context(|| format!("解析 GitHub API 响应失败: {path}"))
}

pub async fn get_user(state: &AppState, token: &str) -> Result<GhUser> {
    gh_get(state, token, "/user").await
}

pub async fn get_primary_email(state: &AppState, token: &str) -> Option<String> {
    let emails: Result<Vec<GhEmail>> = gh_get(state, token, "/user/emails").await;
    match emails {
        Ok(list) => list
            .iter()
            .find(|e| e.primary && e.verified)
            .or_else(|| list.iter().find(|e| e.verified))
            .map(|e| e.email.clone()),
        Err(_) => None,
    }
}

pub async fn list_repos(state: &AppState, token: &str, page: i64) -> Result<(Vec<GhRepo>, bool)> {
    let per_page = 100;
    let list: Vec<GhRepo> = gh_get(
        state,
        token,
        &format!("/user/repos?per_page={per_page}&page={page}&sort=full_name&affiliation=owner,collaborator,organization_member"),
    )
    .await?;
    let has_next = list.len() as i64 == per_page;
    Ok((list, has_next))
}

pub async fn get_repo(state: &AppState, token: &str, owner: &str, repo: &str) -> Result<GhRepo> {
    gh_get(state, token, &format!("/repos/{owner}/{repo}"))
        .await
        .map_err(|e| map_repo_error(e, owner, repo))
}

fn map_repo_error(e: anyhow::Error, owner: &str, repo: &str) -> anyhow::Error {
    if e.to_string().contains("NOT_FOUND_REPO") {
        anyhow::anyhow!(
            "仓库 {owner}/{repo} 不存在，或当前 GitHub 账号无权访问（注意：平台不接受私有仓库）"
        )
    } else {
        e
    }
}

pub async fn list_releases(
    state: &AppState,
    token: &str,
    owner: &str,
    repo: &str,
) -> Result<Vec<GhRelease>> {
    gh_get(
        state,
        token,
        &format!("/repos/{owner}/{repo}/releases?per_page=100"),
    )
    .await
}

/// 读取仓库默认分支根目录的 manifest.json（≤256 KiB）
pub async fn fetch_manifest(
    state: &AppState,
    token: &str,
    owner: &str,
    repo: &str,
    default_branch: &str,
) -> Result<serde_json::Value> {
    let c: GhContent = gh_get(
        state,
        token,
        &format!(
            "/repos/{owner}/{repo}/contents/manifest.json?ref={}",
            urlencoding::encode(default_branch)
        ),
    )
    .await
    .map_err(|e| map_manifest_error(e, owner, repo, default_branch))?;
    let b64 = c.content.ok_or_else(|| {
        coded(
            "manifest_empty",
            serde_json::json!({}),
            "manifest.json 内容为空（文件超过 1MiB 或不可读）",
        )
    })?;
    let bytes = base64_decode_for_github(&b64)?;
    if bytes.len() > MANIFEST_MAX_BYTES {
        return Err(coded(
            "manifest_too_large",
            serde_json::json!({}),
            "manifest.json 超过 256 KiB 上限",
        ));
    }
    serde_json::from_slice(&bytes).map_err(|e| {
        coded(
            "manifest_not_json",
            serde_json::json!({}),
            format!("manifest.json 不是合法 JSON：{e}"),
        )
    })
}

pub async fn fetch_readme(
    state: &AppState,
    token: &str,
    owner: &str,
    repo: &str,
) -> Option<String> {
    let c: GhContent = gh_get(state, token, &format!("/repos/{owner}/{repo}/readme"))
        .await
        .ok()?;
    let b64 = c.content?;
    let bytes = base64_decode_for_github(&b64).ok()?;
    Some(String::from_utf8_lossy(&bytes).chars().take(8000).collect())
}

pub async fn fetch_tree_paths(
    state: &AppState,
    token: &str,
    owner: &str,
    repo: &str,
    default_branch: &str,
) -> Vec<String> {
    #[derive(Deserialize)]
    struct Tree {
        path: String,
        r#type: String,
    }
    #[derive(Deserialize)]
    struct Wrap {
        tree: Vec<Tree>,
    }
    let w: Result<Wrap> = gh_get(
        state,
        token,
        &format!(
            "/repos/{owner}/{repo}/git/trees/{}?recursive=1",
            urlencoding::encode(default_branch)
        ),
    )
    .await;
    match w {
        Ok(w) => w
            .tree
            .into_iter()
            .filter(|t| t.r#type == "blob")
            .take(500)
            .map(|t| t.path)
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// 下载 Release 附件到内存并计算 SHA-256（小写十六进制）；上限由契约常量控制。
pub async fn download_and_hash_asset(state: &AppState, url: &str) -> Result<(Vec<u8>, String)> {
    let resp = state
        .http()
        .get(url)
        .send()
        .await
        .context("下载 Release 附件失败（无法访问附件地址）")?;
    if !resp.status().is_success() {
        anyhow::bail!(
            "下载 Release 附件返回 {}：请确认该 Release 已发布（非 draft）且附件可公开访问",
            resp.status()
        );
    }
    if resp
        .content_length()
        .is_some_and(|length| length > PLUGIN_ZIP_MAX_BYTES)
    {
        anyhow::bail!(
            "插件包超过 {} MiB 上限",
            PLUGIN_ZIP_MAX_BYTES / (1024 * 1024)
        );
    }
    let mut hasher = Sha256::new();
    let mut buf: Vec<u8> = Vec::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("下载附件中断")?;
        if chunk.len() as u64 > PLUGIN_ZIP_MAX_BYTES.saturating_sub(buf.len() as u64) {
            anyhow::bail!(
                "插件包超过 {} MiB 上限",
                PLUGIN_ZIP_MAX_BYTES / (1024 * 1024)
            );
        }
        hasher.update(&chunk);
        buf.extend_from_slice(&chunk);
    }
    Ok((buf, hex::encode(hasher.finalize())))
}

fn truncate(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// GitHub contents API 返回带换行的 base64
fn base64_decode_for_github(s: &str) -> Result<Vec<u8>> {
    use base64::Engine;
    let cleaned: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    base64::engine::general_purpose::STANDARD
        .decode(cleaned)
        .context("base64 解码失败")
}

/// manifest.json 404 时给出具体修复指引
/// 带稳定错误码的开发者校验错误：前端按 code + 当前语言本地化，message 为中文兜底。
/// 通过 anyhow 链传递（downcast_ref 可穿透 context 层）。
#[derive(Debug, Clone)]
pub struct CodedError {
    pub code: &'static str,
    pub params: serde_json::Value,
    pub message: String,
}

impl std::fmt::Display for CodedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CodedError {}

pub fn coded(
    code: &'static str,
    params: serde_json::Value,
    message: impl Into<String>,
) -> anyhow::Error {
    anyhow::Error::new(CodedError {
        code,
        params,
        message: message.into(),
    })
}

fn map_manifest_error(e: anyhow::Error, owner: &str, repo: &str, branch: &str) -> anyhow::Error {
    if e.to_string().contains("NOT_FOUND_FILE") {
        let message = format!(
            "仓库 {owner}/{repo} 的默认分支（{branch}）根目录没有 manifest.json。\
             请在仓库根目录添加插件清单 manifest.json，推送后重新校验"
        );
        coded(
            "manifest_missing",
            serde_json::json!({ "owner": owner, "repo": repo, "branch": branch }),
            message,
        )
    } else {
        e
    }
}
