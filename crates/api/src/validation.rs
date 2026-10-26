use std::sync::LazyLock;

use regex_lite::Regex;
use serde::{Deserialize, Serialize};

// —— 宿主侧契约常量（第 6 节）——
pub const PLUGIN_ID_PREFIX: &str = "wini_plugin_";
pub const OFFICIAL_DEVELOPER_ID: &str = "winiofficial";
pub const RESERVED_PLUGIN_IDS: &[&str] = &["wini_plugin_countdown"];
pub const MAX_SOURCES: i64 = 512;
pub const MAX_PLUGINS_PER_SOURCE: i64 = 2048;
pub const MAX_TOTAL_PLUGINS: i64 = 8192;
pub const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
pub const MANIFEST_MAX_BYTES: usize = 256 * 1024;
pub const PLUGIN_ZIP_MAX_BYTES: u64 = 64 * 1024 * 1024;

static RE_DEVELOPER_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z][a-z0-9_]*$").unwrap());
static RE_PLUGIN_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z][a-z0-9_]*$").unwrap());
static RE_REPO_SEGMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z0-9_.-]+$").unwrap());
static RE_PERMISSION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-z][a-z0-9_]*$").unwrap());
static RE_SHA256: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[0-9a-f]{64}$").unwrap());
// 严格 SemVer 2.0：数字段无前导零，允许 prerelease / build 元数据
static RE_SEMVER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$")
        .unwrap()
});

pub fn valid_developer_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && RE_DEVELOPER_ID.is_match(s)
}

pub fn valid_plugin_id(s: &str) -> bool {
    let Some(name) = s.strip_prefix(PLUGIN_ID_PREFIX) else {
        return false;
    };
    !name.is_empty() && name.len() + PLUGIN_ID_PREFIX.len() <= 80 && RE_PLUGIN_NAME.is_match(name)
}

pub fn is_reserved_plugin_id(s: &str) -> bool {
    RESERVED_PLUGIN_IDS.contains(&s)
}

pub fn valid_semver(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && RE_SEMVER.is_match(s)
}

pub fn valid_sha256(s: &str) -> bool {
    RE_SHA256.is_match(s)
}

/// 解析并校验 repositoryUrl：必须严格等于 https://github.com/{owner}/{repo}
pub fn parse_repo_url(s: &str) -> Result<(String, String), String> {
    let url = url::Url::parse(s).map_err(|_| "repositoryUrl 不是合法 URL".to_string())?;
    if url.scheme() != "https" {
        return Err("repositoryUrl 必须是 https".into());
    }
    if url.host_str() != Some("github.com") {
        return Err("repositoryUrl 域名必须恰好是 github.com".into());
    }
    if url.port().is_some() {
        return Err("repositoryUrl 不能带端口".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("repositoryUrl 不能带 userinfo".into());
    }
    let segs: Vec<&str> = url.path().trim_start_matches('/').split('/').collect();
    if url.path().ends_with('/') || segs.len() != 2 {
        return Err(
            "repositoryUrl 必须恰好是 https://github.com/{owner}/{repo}，无多余段、无尾斜杠".into(),
        );
    }
    for seg in &segs {
        if !RE_REPO_SEGMENT.is_match(seg) || *seg == "." || *seg == ".." || seg.contains('%') {
            return Err(format!("repositoryUrl 路径段非法：{seg}"));
        }
    }
    let repo = segs[1];
    if repo.eq_ignore_ascii_case(".git") || repo.to_ascii_lowercase().ends_with(".git") {
        return Err("repositoryUrl 不能带 .git 后缀".into());
    }
    Ok((segs[0].to_string(), repo.to_string()))
}

/// 校验 downloadUrl：https://github.com/{owner}/{repo}/releases/download/{tag}/{file}.zip
/// owner/repo 必须与 repositoryUrl 一致；tag/file 为单个非空段、不含 ..、不含百分号编码
pub fn validate_download_url(dl: &str, repo_url: &str) -> Result<(), String> {
    let (owner, repo) = parse_repo_url(repo_url)?;
    let url = url::Url::parse(dl).map_err(|_| "downloadUrl 不是合法 URL".to_string())?;
    if url.scheme() != "https" {
        return Err("downloadUrl 必须是 https".into());
    }
    if url.host_str() != Some("github.com") {
        return Err("downloadUrl 域名必须恰好是 github.com".into());
    }
    if url.port().is_some() {
        return Err("downloadUrl 不能带端口".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("downloadUrl 不能带 userinfo".into());
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err("downloadUrl 不能带 query 或 fragment".into());
    }
    let path = url.path();
    if path.ends_with('/') {
        return Err("downloadUrl 不能以斜杠结尾".into());
    }
    let segs: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    if segs.len() != 6 {
        return Err("downloadUrl 路径必须是 /{owner}/{repo}/releases/download/{tag}/{file}".into());
    }
    let [dl_owner, dl_repo, kw1, kw2, tag, file] =
        [segs[0], segs[1], segs[2], segs[3], segs[4], segs[5]];
    if dl_owner != owner || dl_repo != repo {
        return Err("downloadUrl 的 owner/repo 必须与 repositoryUrl 一致".into());
    }
    if kw1 != "releases" || kw2 != "download" {
        return Err("downloadUrl 路径必须是 /releases/download/ 段".into());
    }
    for seg in [tag, file] {
        if seg.is_empty()
            || seg == "."
            || seg == ".."
            || seg.contains('%')
            || seg.contains('/')
            || seg.contains('\\')
        {
            return Err(format!("downloadUrl 的段非法：{seg}"));
        }
    }
    if !file.to_ascii_lowercase().ends_with(".zip") || file.len() <= ".zip".len() {
        return Err("downloadUrl 的文件名必须以 .zip 结尾".into());
    }
    Ok(())
}

/// 权限列表：≤64 项、每项格式合法、互不重复、属于宿主权限表
pub fn validate_permissions(
    perms: &[String],
    known: &std::collections::HashSet<String>,
) -> Result<(), String> {
    if perms.len() > 64 {
        return Err("权限列表最多 64 项".into());
    }
    let mut seen = std::collections::HashSet::new();
    for p in perms {
        if p.is_empty() || p.len() > 80 || !RE_PERMISSION.is_match(p) {
            return Err(format!("权限名非法：{p}"));
        }
        if !seen.insert(p.clone()) {
            return Err(format!("权限重复：{p}"));
        }
        if !known.contains(p) {
            return Err(format!("未知权限（不在宿主权限表中）：{p}"));
        }
    }
    Ok(())
}

/// 权限名格式校验（小写字母开头，仅小写字母/数字/下划线，≤80）
pub fn valid_permission_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 80 && RE_PERMISSION.is_match(name)
}

/// 出索引条目（字段名与顺序即宿主契约）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(rename = "developerId")]
    pub developer_id: String,
    #[serde(rename = "repositoryUrl")]
    pub repository_url: String,
    #[serde(rename = "downloadUrl")]
    pub download_url: String,
    pub sha256: String,
    #[serde(rename = "minHostVersion")]
    pub min_host_version: String,
    #[serde(rename = "contractVersion")]
    pub contract_version: i64,
    pub permissions: Vec<String>,
    pub description: String,
}

impl CatalogEntry {
    /// 出索引时的全字段双重校验，任一不符不得进入索引
    pub fn validate(&self, known: &std::collections::HashSet<String>) -> Result<(), String> {
        if !valid_developer_id(&self.developer_id) {
            return Err(format!("developerId 非法：{}", self.developer_id));
        }
        if self.name.is_empty() || self.name.chars().count() > 200 {
            return Err(format!("name 非法（插件 {}）", self.id));
        }
        if self.description.chars().count() > 8000 {
            return Err(format!("description 超 8000 字符（插件 {}）", self.id));
        }
        if !valid_plugin_id(&self.id) || is_reserved_plugin_id(&self.id) {
            return Err(format!("插件 id 非法或保留：{}", self.id));
        }
        if !valid_semver(&self.version) {
            return Err(format!("version 不是严格 SemVer：{}", self.version));
        }
        if !valid_semver(&self.min_host_version) {
            return Err(format!(
                "minHostVersion 不是严格 SemVer：{}",
                self.min_host_version
            ));
        }
        if self.contract_version < 1 {
            return Err(format!(
                "contractVersion 必须 ≥1：{}",
                self.contract_version
            ));
        }
        if !valid_sha256(&self.sha256) {
            return Err(format!("sha256 非法（插件 {}）", self.id));
        }
        validate_permissions(&self.permissions, known)?;
        validate_download_url(&self.download_url, &self.repository_url)?;
        parse_repo_url(&self.repository_url)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known() -> std::collections::HashSet<String> {
        ["notifications", "storage", "network", "process"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    #[test]
    fn developer_id_rules() {
        assert!(valid_developer_id("abc"));
        assert!(valid_developer_id("a1_b2"));
        assert!(valid_developer_id("winiofficial"));
        assert!(!valid_developer_id("Abc"));
        assert!(!valid_developer_id("1abc"));
        assert!(!valid_developer_id("a-b"));
        assert!(!valid_developer_id(""));
        assert!(!valid_developer_id(&"a".repeat(65)));
    }

    #[test]
    fn plugin_id_rules() {
        assert!(valid_plugin_id("wini_plugin_hello"));
        assert!(valid_plugin_id("wini_plugin_a1"));
        assert!(!valid_plugin_id("wini_plugin_Hello"));
        assert!(!valid_plugin_id("wini_plugin_1a"));
        assert!(!valid_plugin_id("wini_plugin_"));
        assert!(!valid_plugin_id("plugin_hello"));
        assert!(!valid_plugin_id(&format!("wini_plugin_{}", "a".repeat(80))));
        assert!(is_reserved_plugin_id("wini_plugin_countdown"));
    }

    #[test]
    fn semver_rules() {
        assert!(valid_semver("1.0.0"));
        assert!(valid_semver("0.1.0"));
        assert!(valid_semver("10.20.30"));
        assert!(valid_semver("1.0.0-beta.1"));
        assert!(valid_semver("1.0.0+build.5"));
        assert!(!valid_semver("01.0.0"));
        assert!(!valid_semver("1.0.00"));
        assert!(!valid_semver("1.0"));
        assert!(!valid_semver("1.0.0.0"));
        assert!(!valid_semver("v1.0.0"));
        assert!(!valid_semver("1..0"));
    }

    #[test]
    fn repo_url_rules() {
        assert_eq!(
            parse_repo_url("https://github.com/alice/my-repo").unwrap(),
            ("alice".into(), "my-repo".into())
        );
        assert!(parse_repo_url("https://github.com/alice/my.repo").is_ok());
        assert!(parse_repo_url("https://github.com/alice/my-repo/").is_err());
        assert!(parse_repo_url("https://github.com/alice/my-repo.git").is_err());
        assert!(parse_repo_url("https://github.com/alice").is_err());
        assert!(parse_repo_url("https://github.com/alice/my-repo/extra").is_err());
        assert!(parse_repo_url("http://github.com/alice/my-repo").is_err());
        assert!(parse_repo_url("https://gitlab.com/alice/my-repo").is_err());
        assert!(parse_repo_url("https://api.github.com/alice/my-repo").is_err());
        assert!(parse_repo_url("https://github.com:8443/alice/my-repo").is_err());
        assert!(parse_repo_url("https://user:pw@github.com/alice/my-repo").is_err());
    }

    #[test]
    fn download_url_rules() {
        let repo = "https://github.com/alice/my-repo";
        assert!(validate_download_url(
            "https://github.com/alice/my-repo/releases/download/v1.0.0/wini_plugin_demo.zip",
            repo
        )
        .is_ok());
        // 其他仓库
        assert!(validate_download_url(
            "https://github.com/bob/my-repo/releases/download/v1.0.0/a.zip",
            repo
        )
        .is_err());
        // %2F 编码的分隔符
        assert!(validate_download_url(
            "https://github.com/alice/my-repo/releases/download/v1%2Fevil/a.zip",
            repo
        )
        .is_err());
        // %5C
        assert!(validate_download_url(
            "https://github.com/alice/my-repo/releases/download/v1%5Cevil/a.zip",
            repo
        )
        .is_err());
        // .. 穿越
        assert!(validate_download_url(
            "https://github.com/alice/my-repo/releases/download/../a.zip",
            repo
        )
        .is_err());
        // 非 zip
        assert!(validate_download_url(
            "https://github.com/alice/my-repo/releases/download/v1/a.tar.gz",
            repo
        )
        .is_err());
        // query / fragment / http / 深层路径
        assert!(validate_download_url(
            "https://github.com/alice/my-repo/releases/download/v1/a.zip?x=1",
            repo
        )
        .is_err());
        assert!(validate_download_url(
            "https://github.com/alice/my-repo/releases/download/v1/a.zip#frag",
            repo
        )
        .is_err());
        assert!(validate_download_url(
            "http://github.com/alice/my-repo/releases/download/v1/a.zip",
            repo
        )
        .is_err());
        assert!(validate_download_url(
            "https://github.com/alice/my-repo/releases/download/v1/sub/a.zip",
            repo
        )
        .is_err());
        // objects.githubusercontent.com 中转域名不允许
        assert!(
            validate_download_url("https://objects.githubusercontent.com/alice/a.zip", repo)
                .is_err()
        );
    }

    #[test]
    fn permission_rules() {
        let k = known();
        assert!(validate_permissions(&["notifications".into()], &k).is_ok());
        assert!(validate_permissions(&[], &k).is_ok());
        assert!(
            validate_permissions(&["notifications".into(), "notifications".into()], &k).is_err()
        );
        assert!(validate_permissions(&["unknown-perm".into()], &k).is_err());
        assert!(validate_permissions(&["NotLower".into()], &k).is_err());
        let many = vec!["network".to_string(); 65];
        assert!(validate_permissions(&many, &k).is_err());
    }

    #[test]
    fn entry_validation() {
        let k = known();
        let e = CatalogEntry {
            id: "wini_plugin_demo".into(),
            name: "Demo".into(),
            version: "1.0.0".into(),
            developer_id: "alice".into(),
            repository_url: "https://github.com/alice/my-repo".into(),
            download_url:
                "https://github.com/alice/my-repo/releases/download/v1.0.0/wini_plugin_demo.zip"
                    .into(),
            sha256: "a".repeat(64),
            min_host_version: "1.0.0".into(),
            contract_version: 1,
            permissions: vec!["notifications".into()],
            description: String::new(),
        };
        assert!(e.validate(&k).is_ok());
        let mut bad = e.clone();
        bad.sha256 = "ABC".repeat(21); // 大写且长度错
        assert!(bad.validate(&k).is_err());
        let mut bad = e.clone();
        bad.name = "".into();
        assert!(bad.validate(&k).is_err());
        let mut bad = e.clone();
        bad.id = "wini_plugin_countdown".into();
        assert!(bad.validate(&k).is_err());
    }

    #[test]
    fn catalog_entry_json_field_order() {
        let e = CatalogEntry {
            id: "wini_plugin_demo".into(),
            name: "Demo".into(),
            version: "1.0.0".into(),
            developer_id: "alice".into(),
            repository_url: "https://github.com/alice/my-repo".into(),
            download_url:
                "https://github.com/alice/my-repo/releases/download/v1.0.0/wini_plugin_demo.zip"
                    .into(),
            sha256: "0".repeat(64),
            min_host_version: "1.0.0".into(),
            contract_version: 1,
            permissions: vec!["notifications".into()],
            description: "d".into(),
        };
        let s = serde_json::to_string(&e).unwrap();
        let expected_prefix = "{\"id\":\"wini_plugin_demo\",\"name\":\"Demo\",\"version\":\"1.0.0\",\"developerId\":\"alice\",\"repositoryUrl\":\"https://github.com/alice/my-repo\",\"downloadUrl\":";
        assert!(s.starts_with(expected_prefix), "unexpected: {s}");
        assert!(s.contains("\"minHostVersion\":\"1.0.0\",\"contractVersion\":1,\"permissions\":[\"notifications\"],\"description\":\"d\"}"));
    }
}
