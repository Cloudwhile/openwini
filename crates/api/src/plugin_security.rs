use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkDisclosure {
    pub origins: Vec<String>,
    pub purpose: String,
    pub data_categories: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
}

fn text(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty()
        && value.encode_utf16().count() <= maximum
        && value
            .chars()
            .all(|c| !c.is_control() || matches!(c, '\n' | '\r' | '\t'))
}

pub fn network(manifest: &Value) -> Result<Option<NetworkDisclosure>, String> {
    match manifest.get("network").filter(|value| !value.is_null()) {
        None => Ok(None),
        Some(value) => serde_json::from_value(value.clone())
            .map(Some)
            .map_err(|_| "清单 network 格式无效".into()),
    }
}

pub fn validate_network(
    network: Option<&NetworkDisclosure>,
    permissions: &[String],
) -> Result<(), String> {
    let Some(network) = network else {
        return Ok(());
    };
    let mut origins = HashSet::new();
    let mut categories = HashSet::new();
    if !permissions.iter().any(|p| p == "network")
        || !(1..=16).contains(&network.origins.len())
        || !(1..=16).contains(&network.data_categories.len())
        || !text(&network.purpose, 1000)
        || network
            .notice
            .as_ref()
            .is_some_and(|notice| !text(notice, 65536))
        || network
            .data_categories
            .iter()
            .any(|category| !text(category, 128) || !categories.insert(category))
    {
        return Err("插件网络披露无效".into());
    }
    for origin in &network.origins {
        let uri = url::Url::parse(origin).map_err(|_| "网络 origin 无效")?;
        if origin.len() > 2048
            || origin.trim() != origin
            || origin.contains(['?', '#', '\\', '@'])
            || origin.chars().any(char::is_control)
            || uri.scheme() != "https"
            || uri.port().is_some()
            || !uri.username().is_empty()
            || uri.password().is_some()
            || uri.host_str().is_none()
            || uri.path() != "/"
            || uri.query().is_some()
            || uri.fragment().is_some()
            || !origins.insert(uri.origin().ascii_serialization().to_ascii_lowercase())
        {
            return Err("网络 origin 必须是互不重复的 HTTPS 地址且不含路径或凭据".into());
        }
    }
    Ok(())
}

pub fn validate(manifest: &Value, permissions: &[String]) -> Result<(), String> {
    if let Some(terms) = manifest.get("terms").filter(|value| !value.is_null()) {
        let revision = terms
            .get("revision")
            .and_then(Value::as_str)
            .ok_or("条款 revision 无效")?;
        let body = terms
            .get("text")
            .and_then(Value::as_str)
            .ok_or("条款 text 无效")?;
        if !text(revision, 128) || !text(body, 65536) || body.len() > 65536 {
            return Err("插件条款无效".into());
        }
    }
    validate_network(network(manifest)?.as_ref(), permissions)
}
