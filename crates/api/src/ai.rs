use anyhow::{Context as _, Result};
use serde_json::{json, Value};

use crate::state::AppState;

/// 触发 AI 审核（异步任务内调用）：失败只标记 ai_state=failed，绝不阻塞申请流程
pub async fn run_and_store(state: AppState, submission_id: i64) {
    if let Err(e) = run_and_store_inner(&state, submission_id).await {
        tracing::error!("AI 审核任务失败（submission {submission_id}）: {e:#}");
        let _ = sqlx::query(
            "UPDATE submissions SET ai_state='failed',
                ai_report_json = jsonb_build_object('error','AI 审核未完成，需人工复核')
             WHERE id=$1 AND ai_state='skipped'",
        )
        .bind(submission_id)
        .execute(state.pool())
        .await;
    }
}

async fn run_and_store_inner(state: &AppState, submission_id: i64) -> Result<()> {
    let enabled = state
        .site_config_plain("ai_enabled")
        .await
        .unwrap_or_else(|| "false".into());
    if enabled != "true" {
        return Ok(()); // 默认关闭：不做任何事
    }
    let base_url = state
        .site_config_plain("ai_base_url")
        .await
        .unwrap_or_default();
    let model = state
        .site_config_plain("ai_model")
        .await
        .unwrap_or_default();
    let api_key = state
        .site_config_plain("ai_api_key")
        .await
        .unwrap_or_default();
    let timeout_sec: u64 = state
        .site_config_plain("ai_timeout_sec")
        .await
        .unwrap_or_else(|| "60".into())
        .parse()
        .unwrap_or(60);
    if base_url.is_empty() || model.is_empty() || api_key.is_empty() {
        // 配置不完整视为故障：标记需人工复核，不阻塞
        sqlx::query(
            "UPDATE submissions SET ai_state='failed',
                ai_report_json = jsonb_build_object('error','AI 审核未完成，需人工复核（配置不完整）')
             WHERE id=$1",
        )
        .bind(submission_id)
        .execute(state.pool())
        .await?;
        return Ok(());
    }

    // 收集输入：清单全文、权限、仓库文件列表、README、Release 信息
    #[derive(sqlx::FromRow)]
    struct Row {
        manifest_json: Value,
        permissions: Vec<String>,
        repository_url: String,
        plugin_id: String,
        submitted_by: i64,
    }
    let row = sqlx::query_as::<_, Row>(
        "SELECT s.manifest_json, s.permissions, p.repository_url, p.plugin_id, s.submitted_by
         FROM submissions s JOIN plugins p ON p.id = s.plugin_id
         WHERE s.id = $1",
    )
    .bind(submission_id)
    .fetch_optional(state.pool())
    .await?
    .context("申请不存在")?;

    let token = developer_github_token(state, row.submitted_by).await?;

    let (owner, repo) = crate::validation::parse_repo_url(&row.repository_url)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let gh_repo = crate::github::get_repo(state, &token, &owner, &repo).await?;
    let branch = gh_repo
        .default_branch
        .clone()
        .unwrap_or_else(|| "main".into());
    let tree = crate::github::fetch_tree_paths(state, &token, &owner, &repo, &branch).await;
    let readme = crate::github::fetch_readme(state, &token, &owner, &repo).await;
    let releases = crate::github::list_releases(state, &token, &owner, &repo).await?;
    let release_summary: Vec<Value> = releases
        .iter()
        .take(10)
        .map(|r| {
            json!({
                "tag": r.tag_name,
                "prerelease": r.prerelease,
                "assets": r.assets.iter().map(|a| a.name.clone()).collect::<Vec<_>>(),
            })
        })
        .collect();

    // 输入只含公开仓库信息与清单，绝不含 GitHub Token、SMTP 密码、API Key、用户邮箱
    let user_content = json!({
        "manifest": row.manifest_json,
        "permissions": row.permissions,
        "repo_files": tree,
        "readme": readme.unwrap_or_default(),
        "releases": release_summary,
    })
    .to_string();

    let system_prompt = "你是 WindowsIsland 桌面客户端插件商店的审核助手。请基于给定的插件清单、权限列表、仓库文件列表、README 与 Release 信息进行风险审查。\n\
        审查要点：1) 高危权限（process、network、filesystem、automation、native）与插件描述的功能是否匹配；2) 描述与仓库内容是否相符；3) 清单字段是否异常。\n\
        只输出一个 JSON 对象，不要输出其他文字，格式：\n\
        {\"risk_flags\": [\"风险点1\", ...], \"recommendation\": \"approve|reject|manual\", \"reasons\": \"理由（中文）\"}\n\
        recommendation 三选一：approve=建议通过，reject=建议退回，manual=需人工复核。你的结论只作为管理员参考，不会自动生效。";

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_sec))
        .build()?;
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&json!({
            "model": model,
            "temperature": 0.1,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_content},
            ]
        }))
        .send()
        .await
        .context("AI 接口请求失败")?;
    if !resp.status().is_success() {
        anyhow::bail!("AI 接口返回 {}", resp.status());
    }
    let body: Value = resp.json().await.context("解析 AI 响应失败")?;
    let content = body
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let parsed = parse_ai_output(&content);

    sqlx::query("UPDATE submissions SET ai_state='ok', ai_report_json=$1 WHERE id=$2")
        .bind(&parsed)
        .bind(submission_id)
        .execute(state.pool())
        .await?;
    Ok(())
}

async fn developer_github_token(state: &AppState, dev_row_id: i64) -> Result<String> {
    let enc: Option<(Option<String>,)> =
        sqlx::query_as("SELECT github_token_encrypted FROM developers WHERE id=$1")
            .bind(dev_row_id)
            .fetch_optional(state.pool())
            .await?;
    let enc = enc
        .and_then(|(e,)| e)
        .context("开发者未连接 GitHub 授权，无法读取仓库信息做 AI 审核")?;
    Ok(crate::crypto::decrypt(&crate::crypto::current_key(), &enc)?)
}

fn parse_ai_output(content: &str) -> Value {
    let start = content.find('{');
    let end = content.rfind('}');
    let extracted = match (start, end) {
        (Some(s), Some(e)) if e > s => &content[s..=e],
        _ => {
            return json!({
                "riskFlags": [],
                "recommendation": "manual",
                "reasons": "AI 输出无法解析，需人工复核",
                "raw": truncate_chars(content, 500),
            })
        }
    };
    match serde_json::from_str::<Value>(extracted) {
        Ok(v) => {
            let rec = v
                .get("recommendation")
                .and_then(|x| x.as_str())
                .unwrap_or("manual")
                .to_string();
            let rec = if ["approve", "reject", "manual"].contains(&rec.as_str()) {
                rec
            } else {
                "manual".into()
            };
            json!({
                "riskFlags": v.get("risk_flags").and_then(|x| x.as_array()).cloned().unwrap_or_default(),
                "recommendation": rec,
                "reasons": v.get("reasons").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                "raw": truncate_chars(content, 500),
            })
        }
        Err(_) => json!({
            "riskFlags": [],
            "recommendation": "manual",
            "reasons": "AI 输出无法解析，需人工复核",
            "raw": truncate_chars(content, 500),
        }),
    }
}

fn truncate_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_plain_json() {
        let v = parse_ai_output(
            r#"{"risk_flags":["高危权限 network 与功能描述可能不匹配"],"recommendation":"manual","reasons":"描述未说明网络用途"}"#,
        );
        assert_eq!(v["recommendation"], "manual");
        assert_eq!(v["riskFlags"][0], "高危权限 network 与功能描述可能不匹配");
    }

    #[test]
    fn parse_json_with_prose() {
        let v = parse_ai_output("好的，以下是审查结果：\n```json\n{\"risk_flags\":[],\"recommendation\":\"approve\",\"reasons\":\"无风险\"}\n```");
        assert_eq!(v["recommendation"], "approve");
    }

    #[test]
    fn parse_garbage_falls_to_manual() {
        let v = parse_ai_output("我不会输出 JSON");
        assert_eq!(v["recommendation"], "manual");
    }

    #[test]
    fn parse_unknown_recommendation_falls_to_manual() {
        let v = parse_ai_output(r#"{"risk_flags":[],"recommendation":"yolo","reasons":""}"#);
        assert_eq!(v["recommendation"], "manual");
    }
}
