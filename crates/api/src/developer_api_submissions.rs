#[derive(Deserialize)]
struct PageQuery {
    page: Option<i64>,
}

async fn list_repos(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<PageQuery>,
) -> ApiResult<Json<Value>> {
    ensure_not_banned(&user)?;
    let token = user.github_token()?;
    let page = q.page.unwrap_or(1).max(1);
    let (repos, has_next) = crate::github::list_repos(&state, &token, page)
        .await
        .map_err(|e| AppError::BadRequest(format!("拉取仓库列表失败：{e}")))?;
    let items: Vec<Value> = repos
        .iter()
        .map(|r| {
            json!({
                "fullName": r.full_name,
                "url": r.html_url,
                "private": r.private,
                "isAdmin": r.permissions.as_ref().map(|p| p.admin).unwrap_or(false),
                "description": r.description,
                "updatedAt": r.updated_at,
            })
        })
        .collect();
    Ok(Json(
        json!({ "items": items, "page": page, "hasNext": has_next }),
    ))
}

#[derive(Deserialize)]
struct RepoBody {
    #[serde(rename = "repositoryUrl")]
    repository_url: String,
}

async fn validate_repo(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<RepoBody>,
) -> ApiResult<Json<Value>> {
    ensure_not_banned(&user)?;
    let token = user.github_token()?;
    let result = sub::validate_repository(&state, &token, &body.repository_url)
        .await
        .map_err(coded_bad_request)?;
    Ok(Json(result))
}

async fn manifest_preview(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<RepoBody>,
) -> ApiResult<Json<Value>> {
    ensure_not_banned(&user)?;
    let token = user.github_token()?;
    let result = sub::manifest_preview(&state, &token, &body.repository_url)
        .await
        .map_err(coded_bad_request)?;
    Ok(Json(result))
}

// —— 插件 CRUD 与申请流程 ——

#[derive(Deserialize)]
struct PluginBody {
    #[serde(rename = "pluginId")]
    plugin_id: String,
    #[serde(rename = "repositoryUrl")]
    repository_url: String,
    #[serde(rename = "homepageUrl")]
    homepage_url: Option<String>,
    #[serde(rename = "docsUrl")]
    docs_url: Option<String>,
}

#[derive(Deserialize)]
struct ListQuery {
    status: Option<String>,
    q: Option<String>,
    page: Option<i64>,
}

async fn create_plugin(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<PluginBody>,
) -> ApiResult<Json<Value>> {
    ensure_not_banned(&user)?;
    ensure_not_system(&user)?;
    let plugin_id = body.plugin_id.trim().to_string();
    if !crate::validation::valid_plugin_id(&plugin_id) {
        return Err(AppError::BadRequest(
            "插件标识必须形如 wini_plugin_<name>：以 wini_plugin_ 开头，name 以小写字母开头且仅含小写字母/数字/下划线，总长不超过 80".into(),
        ));
    }
    if crate::validation::is_reserved_plugin_id(&plugin_id) {
        return Err(AppError::Conflict(format!(
            "插件标识 {plugin_id} 是内置保留标识"
        )));
    }
    crate::validation::parse_repo_url(&body.repository_url).map_err(|e| AppError::BadRequest(e))?;
    check_optional_url(body.homepage_url.as_deref(), "主页链接")?;
    check_optional_url(body.docs_url.as_deref(), "文档链接")?;

    let taken: Option<(i64,)> = sqlx::query_as("SELECT id FROM plugins WHERE plugin_id=$1")
        .bind(&plugin_id)
        .fetch_optional(state.pool())
        .await
        .map_err(anyhow::Error::from)?;
    if taken.is_some() {
        return Err(AppError::Conflict(format!("插件标识 {plugin_id} 已被使用")));
    }

    let mut tx = state.pool().begin().await.map_err(anyhow::Error::from)?;
    sqlx::query(
        "INSERT INTO plugins (developer_id, plugin_id, repository_url, homepage_url, docs_url, status)
         VALUES ($1,$2,$3,$4,$5,'draft')",
    )
    .bind(user.dev.id)
    .bind(&plugin_id)
    .bind(body.repository_url.trim())
    .bind(body.homepage_url.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(body.docs_url.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .execute(&mut *tx)
    .await
    .map_err(anyhow::Error::from)?;
    crate::audit::write(
        &mut tx,
        &user.dev.developer_id,
        "developer",
        "plugin_created",
        Some(&plugin_id),
        None,
        None,
        Some("draft"),
        None,
    )
    .await?;
    tx.commit().await.map_err(anyhow::Error::from)?;
    Ok(Json(json!({ "pluginId": plugin_id })))
}

fn check_optional_url(url: Option<&str>, label: &str) -> ApiResult<()> {
    if let Some(u) = url {
        let u = u.trim();
        if u.is_empty() {
            return Ok(());
        }
        let parsed = url::Url::parse(u)
            .map_err(|_| AppError::BadRequest(format!("{label} 不是合法 URL")))?;
        if parsed.scheme() != "https" && parsed.scheme() != "http" {
            return Err(AppError::BadRequest(format!(
                "{label} 必须是 http/https 链接"
            )));
        }
    }
    Ok(())
}

#[derive(FromRow)]
struct PluginRow {
    id: i64,
    developer_id: i64,
    plugin_id: String,
    repository_url: String,
    homepage_url: Option<String>,
    docs_url: Option<String>,
    status: String,
    current_version: Option<String>,
    current_submission_id: Option<i64>,
    updates_halted: bool,
    updates_halted_reason: Option<String>,
    stop_reason: Option<String>,
}

async fn load_own_plugin(
    state: &AppState,
    user: &CurrentUser,
    plugin_id: &str,
    allow_official_read_for_admin: bool,
) -> ApiResult<PluginRow> {
    let row = sqlx::query_as::<_, PluginRow>("SELECT * FROM plugins WHERE plugin_id = $1")
        .bind(plugin_id)
        .fetch_optional(state.pool())
        .await
        .map_err(anyhow::Error::from)?
        .ok_or_else(|| AppError::NotFound("插件不存在".into()))?;
    // 资源归属校验：只能操作自己名下插件（管理员的个人插件同样受此约束）。
    // allow_official_read_for_admin：仅详情读取场景，管理员可读官方身份（winiofficial）插件
    let mut allowed = row.developer_id == user.dev.id;
    if !allowed && allow_official_read_for_admin && user.dev.is_admin {
        let is_official_row: (bool,) =
            sqlx::query_as("SELECT is_system FROM developers WHERE id = $1")
                .bind(row.developer_id)
                .fetch_one(state.pool())
                .await
                .map_err(anyhow::Error::from)?;
        allowed = is_official_row.0;
    }
    if !allowed {
        return Err(AppError::Forbidden("只能操作自己名下的插件".into()));
    }
    Ok(row)
}

async fn list_plugins(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<Value>> {
    let page_size: i64 = 20;
    let page = q.page.unwrap_or(1).max(1);
    let pattern = format!(
        "%{}%",
        q.q.clone()
            .unwrap_or_default()
            .replace('%', "\\%")
            .replace('_', "\\_")
    );
    let status = q.status.clone().unwrap_or_default();
    let rows: Vec<(
        String,
        String,
        Option<String>,
        bool,
        Option<String>,
        String,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        "SELECT p.plugin_id, p.status, p.current_version, p.updates_halted, p.stop_reason,
                COALESCE((SELECT s.version FROM submissions s
                          WHERE s.plugin_id = p.id AND s.status='pending'
                          ORDER BY s.created_at DESC LIMIT 1), ''),
                p.updated_at
         FROM plugins p
         WHERE p.developer_id = $1
           AND ($2 = '' OR p.status = $2)
           AND (p.plugin_id ILIKE $3 OR COALESCE(p.current_version,'') ILIKE $3)
         ORDER BY p.updated_at DESC
         LIMIT $4 OFFSET $5",
    )
    .bind(user.dev.id)
    .bind(&status)
    .bind(&pattern)
    .bind(page_size)
    .bind((page - 1) * page_size)
    .fetch_all(state.pool())
    .await
    .map_err(anyhow::Error::from)?;
    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM plugins p
         WHERE p.developer_id = $1 AND ($2 = '' OR p.status = $2)
           AND (p.plugin_id ILIKE $3 OR COALESCE(p.current_version,'') ILIKE $3)",
    )
    .bind(user.dev.id)
    .bind(&status)
    .bind(&pattern)
    .fetch_one(state.pool())
    .await
    .map_err(anyhow::Error::from)?;
    let items: Vec<Value> = rows
        .into_iter()
        .map(|(plugin_id, status, version, halted, stop_reason, pending_version, updated)| {
            let display_status = crate::status::display_status(&status, halted, stop_reason.is_some());
            json!({
                "pluginId": plugin_id,
                "status": display_status,
                "version": version.unwrap_or_default(),
                "pendingVersion": if pending_version.is_empty() { Value::Null } else { json!(pending_version) },
                "stopReason": stop_reason,
                "updatedAt": updated,
            })
        })
        .collect();
    Ok(Json(
        json!({ "items": items, "total": total.0, "page": page, "pageSize": page_size }),
    ))
}

async fn plugin_detail(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(plugin_id): Path<String>,
) -> ApiResult<Json<Value>> {
    let p = load_own_plugin(&state, &user, &plugin_id, true).await?;
    let submissions: Vec<Value> = sqlx::query(
        "SELECT id, version, status, is_new_version, description, permissions, download_url, sha256,
                min_host_version, contract_version, review_reason, ai_state, ai_report_json,
                created_at, reviewed_at
         FROM submissions WHERE plugin_id = $1 ORDER BY created_at DESC",
    )
    .bind(p.id)
    .fetch_all(state.pool())
    .await
    .map_err(anyhow::Error::from)?
    .into_iter()
    .map(|row: sqlx::postgres::PgRow| {
        let id: i64 = row.get("id");
        let ai_state: String = row.get("ai_state");
        let ai_report: Option<Value> = row.get("ai_report_json");
        // 开发者只能看到 AI 报告的公开建议部分（风险标记），内部推理不展示
        let ai_public = ai_report.map(|r| json!({
            "riskFlags": r.get("riskFlags").cloned().unwrap_or_else(|| json!([])),
            "state": ai_state,
        }));
        json!({
            "id": id,
            "version": row.get::<String, _>("version"),
            "status": row.get::<String, _>("status"),
            "isNewVersion": row.get::<bool, _>("is_new_version"),
            "description": row.get::<String, _>("description"),
            "permissions": row.get::<Vec<String>, _>("permissions"),
            "downloadUrl": row.get::<String, _>("download_url"),
            "sha256": row.get::<String, _>("sha256"),
            "minHostVersion": row.get::<String, _>("min_host_version"),
            "contractVersion": row.get::<i32, _>("contract_version"),
            "reviewReason": row.get::<Option<String>, _>("review_reason"),
            "ai": ai_public,
            "createdAt": row.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
            "reviewedAt": row.get::<Option<chrono::DateTime<chrono::Utc>>, _>("reviewed_at"),
            "isCurrent": p.current_submission_id == Some(id),
        })
    })
    .collect();
    let audits: Vec<Value> = sqlx::query(
        "SELECT event_id, actor, action, from_status, to_status, reason, created_at
         FROM audit_logs WHERE plugin_id = $1 ORDER BY created_at DESC LIMIT 100",
    )
    .bind(&p.plugin_id)
    .fetch_all(state.pool())
    .await
    .map_err(anyhow::Error::from)?
    .into_iter()
    .map(|row: sqlx::postgres::PgRow| {
        json!({
            "eventId": row.get::<String, _>("event_id"),
            "actor": row.get::<String, _>("actor"),
            "action": row.get::<String, _>("action"),
            "fromStatus": row.get::<Option<String>, _>("from_status"),
            "toStatus": row.get::<Option<String>, _>("to_status"),
            "reason": row.get::<Option<String>, _>("reason"),
            "createdAt": row.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
        })
    })
    .collect();
    Ok(Json(json!({
        "pluginId": p.plugin_id,
        "repositoryUrl": p.repository_url,
        "homepageUrl": p.homepage_url,
        "docsUrl": p.docs_url,
        "status": p.status,
        "displayStatus": crate::status::display_status(&p.status, p.updates_halted, p.stop_reason.is_some()),
        "currentVersion": p.current_version,
        "updatesHalted": p.updates_halted,
        "updatesHaltedReason": p.updates_halted_reason,
        "stopReason": p.stop_reason,
        "submissions": submissions,
        "timeline": audits,
        "canEdit": crate::status::can_edit(&p.status),
        "canCancel": crate::status::can_cancel(&p.status),
        "canSubmit": crate::status::can_submit(&p.status),
        "canPause": crate::status::can_pause(&p.status),
        "canResumeSelf": crate::status::can_resume_self(&p.status),
        "canUnlist": crate::status::can_unlist(&p.status),
        "canSubmitNewVersion": crate::status::can_submit_new_version(&p.status, p.updates_halted),
    })))
}

#[derive(Deserialize)]
struct EditBody {
    #[serde(rename = "repositoryUrl")]
    repository_url: Option<String>,
    #[serde(rename = "homepageUrl")]
    homepage_url: Option<String>,
    #[serde(rename = "docsUrl")]
    docs_url: Option<String>,
}

/// 编辑申请：draft/pending/rejected 可直接编辑；pending 编辑后保持待审核并更新提交时间
async fn edit_plugin(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(plugin_id): Path<String>,
    Json(body): Json<EditBody>,
) -> ApiResult<Json<Value>> {
    ensure_not_banned(&user)?;
    let p = load_own_plugin(&state, &user, &plugin_id, false).await?;
    if !matches!(
        p.status.as_str(),
        "draft" | "pending" | "rejected" | "unlisted"
    ) {
        return Err(AppError::Forbidden(
            "当前状态不可编辑：已发布插件的修改请走「提交新版本」流程".into(),
        ));
    }
    let repo = match body.repository_url {
        Some(r) if !r.trim().is_empty() => {
            crate::validation::parse_repo_url(r.trim()).map_err(AppError::BadRequest)?;
            let trimmed = r.trim();
            if p.status == "pending" && trimmed != p.repository_url {
                return Err(AppError::Conflict(
                    "待审核申请不能直接更换仓库；请取消申请后修改仓库，再重新提交".into(),
                ));
            }
            trimmed.to_string()
        }
        _ => p.repository_url.clone(),
    };
    let homepage = merge_opt(p.homepage_url.clone(), body.homepage_url);
    let docs = merge_opt(p.docs_url.clone(), body.docs_url);
    check_optional_url(homepage.as_deref(), "主页链接")?;
    check_optional_url(docs.as_deref(), "文档链接")?;

    let mut tx = state.pool().begin().await.map_err(anyhow::Error::from)?;
    sqlx::query(
        "UPDATE plugins SET repository_url=$1, homepage_url=$2, docs_url=$3, updated_at=now() WHERE id=$4",
    )
    .bind(&repo)
    .bind(&homepage)
    .bind(&docs)
    .bind(p.id)
    .execute(&mut *tx)
    .await
    .map_err(anyhow::Error::from)?;
    crate::audit::write(
        &mut tx,
        &user.dev.developer_id,
        "developer",
        "plugin_edited",
        Some(&p.plugin_id),
        None,
        Some(p.status.as_str()),
        Some(p.status.as_str()),
        None,
    )
    .await?;
    tx.commit().await.map_err(anyhow::Error::from)?;
    Ok(Json(json!({ "ok": true })))
}

fn merge_opt(old: Option<String>, new: Option<String>) -> Option<String> {
    match new {
        Some(v) => {
            let v = v.trim().to_string();
            if v.is_empty() {
                None
            } else {
                Some(v)
            }
        }
        None => old,
    }
}

#[derive(Deserialize)]
struct SubmitBody {
    version: String,
    description: String,
    #[serde(rename = "descriptionEn", default)]
    description_en: Option<String>,
    #[serde(rename = "repositoryUrl")]
    repository_url: Option<String>,
    #[serde(rename = "homepageUrl")]
    homepage_url: Option<String>,
    #[serde(rename = "docsUrl")]
    docs_url: Option<String>,
}

/// 提交申请 / 重新提交：draft、rejected、cancelled → pending
async fn submit_plugin(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(plugin_id): Path<String>,
    Json(body): Json<SubmitBody>,
) -> ApiResult<Json<Value>> {
    ensure_not_banned(&user)?;
    ensure_not_system(&user)?;
    let p = load_own_plugin(&state, &user, &plugin_id, false).await?;
    if !matches!(
        p.status.as_str(),
        "draft" | "rejected" | "cancelled" | "pending" | "unlisted"
    ) {
        return Err(AppError::Conflict("当前状态不能提交申请".into()));
    }
    let repo = match body.repository_url {
        Some(r) if !r.trim().is_empty() => {
            let r = r.trim().to_string();
            crate::validation::parse_repo_url(&r).map_err(AppError::BadRequest)?;
            r
        }
        _ => p.repository_url.clone(),
    };
    let homepage = merge_opt(p.homepage_url.clone(), body.homepage_url);
    let docs = merge_opt(p.docs_url.clone(), body.docs_url);

    do_submit(
        state,
        user,
        p,
        &repo,
        homepage,
        docs,
        &body.version,
        body.description.clone(),
        body.description_en.as_deref(),
        false,
    )
    .await
}

/// 新版本申请：挂在已发布插件下的独立 pending 申请
async fn submit_new_version(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(plugin_id): Path<String>,
    Json(body): Json<SubmitBody>,
) -> ApiResult<Json<Value>> {
    ensure_not_banned(&user)?;
    ensure_not_system(&user)?;
    let p = load_own_plugin(&state, &user, &plugin_id, false).await?;
    if p.status != "published" {
        return Err(AppError::Conflict("只有已发布的插件才能提交新版本".into()));
    }
    if p.updates_halted {
        return Err(AppError::Conflict(
            "该插件已被管理员停止更新，暂不能提交新版本；待恢复更新后再提交".into(),
        ));
    }
    // 仓库地址发布后不可改
    if let Some(r) = &body.repository_url {
        if r.trim() != p.repository_url {
            return Err(AppError::Conflict("仓库地址在发布后不可修改".into()));
        }
    }
    let homepage = merge_opt(p.homepage_url.clone(), body.homepage_url);
    let docs = merge_opt(p.docs_url.clone(), body.docs_url);
    // 同一插件同时只允许一个 pending 版本申请
    let pending: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM submissions WHERE plugin_id=$1 AND status='pending'")
            .bind(p.id)
            .fetch_optional(state.pool())
            .await
            .map_err(anyhow::Error::from)?;
    if pending.is_some() {
        return Err(AppError::Conflict("该插件已有待审核的版本申请".into()));
    }
    let repo = p.repository_url.clone();
    do_submit(
        state,
        user,
        p,
        &repo,
        homepage,
        docs,
        &body.version,
        body.description.clone(),
        body.description_en.as_deref(),
        true,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn do_submit(
    state: AppState,
    user: CurrentUser,
    p: PluginRow,
    repo: &str,
    homepage: Option<String>,
    docs: Option<String>,
    version: &str,
    description: String,
    description_en: Option<&str>,
    is_new_version: bool,
) -> ApiResult<Json<Value>> {
    let token = user.github_token()?;

    // 同一开发者的已发布插件必须使用同一仓库（宿主源契约：条目 repositoryUrl 与源条目逐字一致）
    let other_repo: Option<(String,)> = sqlx::query_as(
        "SELECT DISTINCT repository_url FROM plugins
         WHERE developer_id = $1 AND status = 'published' AND repository_url <> $2 LIMIT 1",
    )
    .bind(user.dev.id)
    .bind(repo)
    .fetch_optional(state.pool())
    .await
    .map_err(anyhow::Error::from)?;
    if let Some((other,)) = other_repo {
        return Err(AppError::Conflict(format!(
            "你的已发布插件使用仓库 {other}；同一开发者的全部已发布插件必须来自同一 GitHub 仓库（宿主目录源契约要求），请将本插件也放在该仓库"
        )));
    }

    let known = sub::known_permission_set(&state)
        .await
        .map_err(anyhow::Error::from)?;
    let validated = sub::validate_full(
        &state,
        &token,
        &p.plugin_id,
        repo,
        version,
        Some(&description),
        description_en,
        &known,
    )
    .await
    .map_err(|diffs| {
        AppError::BadRequest(json!({ "error": "清单比对未通过", "differences": diffs }).to_string())
    })?;

    let mut tx = state.pool().begin().await.map_err(anyhow::Error::from)?;
    // pending 状态下编辑重提：更新原待审行并刷新提交时间；否则新建申请行
    let existing_pending: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM submissions WHERE plugin_id=$1 AND status='pending'")
            .bind(p.id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(anyhow::Error::from)?;
    let replacing = existing_pending.is_some();
    let sub_id: (i64,) = if let Some((sid,)) = existing_pending {
        sqlx::query_as::<_, (i64,)>(
            "UPDATE submissions SET version=$1, manifest_json=$2, manifest_name=$3, description=$4,
                description_en=$5, permissions=$6, download_url=$7, sha256=$8, min_host_version=$9, contract_version=$10,
                submitted_by=$11, created_at=now(), review_reason=NULL, reviewed_by=NULL, reviewed_at=NULL,
                ai_state='skipped', ai_report_json=NULL
             WHERE id=$12 RETURNING id",
        )
        .bind(&validated.version)
        .bind(&validated.manifest_json)
        .bind(&validated.manifest_name)
        .bind(&validated.description)
        .bind(&validated.description_en)
        .bind(&validated.permissions)
        .bind(&validated.download_url)
        .bind(&validated.sha256)
        .bind(&validated.min_host_version)
        .bind(validated.contract_version)
        .bind(user.dev.id)
        .bind(sid)
        .fetch_one(&mut *tx)
        .await
        .map_err(anyhow::Error::from)?
    } else {
        sqlx::query_as(
            "INSERT INTO submissions
                (plugin_id, version, manifest_json, manifest_name, description, description_en, permissions,
                 download_url, sha256, min_host_version, contract_version, status, is_new_version, submitted_by)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,'pending',$12,$13)
             RETURNING id",
        )
        .bind(p.id)
        .bind(&validated.version)
        .bind(&validated.manifest_json)
        .bind(&validated.manifest_name)
        .bind(&validated.description)
        .bind(&validated.description_en)
        .bind(&validated.permissions)
        .bind(&validated.download_url)
        .bind(&validated.sha256)
        .bind(&validated.min_host_version)
        .bind(validated.contract_version)
        .bind(is_new_version)
        .bind(user.dev.id)
        .fetch_one(&mut *tx)
        .await
        .map_err(anyhow::Error::from)?
    };
    let from_status = p.status.clone();
    let to_status = if is_new_version {
        p.status.clone()
    } else {
        "pending".to_string()
    };
    if !is_new_version {
        // 重新提交：清除历史管理退回原因，避免与普通审核驳回混淆
        sqlx::query("UPDATE plugins SET status='pending', repository_url=$1, homepage_url=$2, docs_url=$3, stop_reason=NULL, updated_at=now() WHERE id=$4")
            .bind(repo)
            .bind(&homepage)
            .bind(&docs)
            .bind(p.id)
            .execute(&mut *tx)
            .await
            .map_err(anyhow::Error::from)?;
    } else {
        sqlx::query(
            "UPDATE plugins SET homepage_url=$1, docs_url=$2, updated_at=now() WHERE id=$3",
        )
        .bind(&homepage)
        .bind(&docs)
        .bind(p.id)
        .execute(&mut *tx)
        .await
        .map_err(anyhow::Error::from)?;
    }
    let action = match (is_new_version, replacing) {
        (true, _) => "new_version_submitted",
        (false, true) => "submission_updated",
        (false, false) => "submission_submitted",
    };
    let event_id = crate::audit::write(
        &mut tx,
        &user.dev.developer_id,
        "developer",
        action,
        Some(&p.plugin_id),
        Some(sub_id.0),
        Some(from_status.as_str()),
        Some(to_status.as_str()),
        None,
    )
    .await?;
    tx.commit().await.map_err(anyhow::Error::from)?;

    // 通知：开发者回执 + 管理员待审（失败不影响申请）
    let vars = MailVars {
        plugin_name: validated.manifest_name.clone(),
        plugin_id: p.plugin_id.clone(),
        version: validated.version.clone(),
        developer: user.dev.display_name.clone(),
        reason: None,
        link: MailVars::link_for(&state, &p.plugin_id),
    };
    mailer::notify_developer_with_event(
        &state,
        user.dev.id,
        "submission_receipt",
        &vars,
        Some(&p.plugin_id),
        Some(&event_id),
    )
    .await;
    let admin_vars = MailVars {
        link: MailVars::admin_queue_link(&state),
        ..vars.clone()
    };
    mailer::notify_admins(
        &state,
        "admin_new_submission",
        &admin_vars,
        Some(&p.plugin_id),
    )
    .await;

    // AI 审核（默认关闭；失败仅标记，不阻塞）
    let st = state.clone();
    tokio::spawn(async move {
        crate::ai::run_and_store(st, sub_id.0).await;
    });

    Ok(Json(
        json!({ "submissionId": sub_id.0, "status": "pending", "eventId": event_id }),
    ))
}

/// 取消申请：draft / pending → cancelled（前端需二次确认）
async fn cancel_plugin(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(plugin_id): Path<String>,
) -> ApiResult<Json<Value>> {
    ensure_not_banned(&user)?;
    let p = load_own_plugin(&state, &user, &plugin_id, false).await?;
    if !matches!(p.status.as_str(), "draft" | "pending") {
        return Err(AppError::Conflict("只有草稿或待审核状态可以取消".into()));
    }
    let mut tx = state.pool().begin().await.map_err(anyhow::Error::from)?;
    sqlx::query("UPDATE submissions SET status='cancelled', reviewed_at=now() WHERE plugin_id=$1 AND status='pending'")
        .bind(p.id)
        .execute(&mut *tx)
        .await
        .map_err(anyhow::Error::from)?;
    sqlx::query("UPDATE plugins SET status='cancelled', updated_at=now() WHERE id=$1")
        .bind(p.id)
        .execute(&mut *tx)
        .await
        .map_err(anyhow::Error::from)?;
    crate::audit::write(
        &mut tx,
        &user.dev.developer_id,
        "developer",
        "submission_cancelled",
        Some(&p.plugin_id),
        None,
        Some(p.status.as_str()),
        Some("cancelled"),
        None,
    )
    .await?;
    tx.commit().await.map_err(anyhow::Error::from)?;
    Ok(Json(json!({ "ok": true, "status": "cancelled" })))
}

/// 暂停（开发者自助）：published → paused，移出公开索引，可随时自行恢复（无需审核）
async fn pause_plugin(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(plugin_id): Path<String>,
) -> ApiResult<Json<Value>> {
    ensure_not_banned(&user)?;
    dev_pause_unlist(state, user, plugin_id, "paused").await
}

/// 下架（开发者自助）：published/paused → unlisted，重新上架需编辑后重新提交审核
async fn unlist_plugin(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(plugin_id): Path<String>,
) -> ApiResult<Json<Value>> {
    ensure_not_banned(&user)?;
    dev_pause_unlist(state, user, plugin_id, "unlisted").await
}

async fn dev_pause_unlist(
    state: AppState,
    user: CurrentUser,
    plugin_id: String,
    to_status: &str,
) -> ApiResult<Json<Value>> {
    let p = load_own_plugin(&state, &user, &plugin_id, false).await?;
    if !matches!(p.status.as_str(), "published" | "paused") {
        return Err(AppError::Conflict(
            "只有已发布或已暂停的插件才能执行该操作".into(),
        ));
    }
    if to_status == "paused" && p.status != "published" {
        return Err(AppError::Conflict("插件已处于暂停状态".into()));
    }
    let mut tx = state.pool().begin().await.map_err(anyhow::Error::from)?;
    let halted_clear = to_status == "unlisted";
    sqlx::query(
        "UPDATE plugins SET status=$1,
            updates_halted = CASE WHEN $2 THEN FALSE ELSE updates_halted END,
            updates_halted_reason = CASE WHEN $2 THEN NULL ELSE updates_halted_reason END,
            updated_at=now()
         WHERE id=$3",
    )
    .bind(to_status)
    .bind(halted_clear)
    .bind(p.id)
    .execute(&mut *tx)
    .await
    .map_err(anyhow::Error::from)?;
    // 排队中的新版本申请一并退回，避免暂停/下架期间被审批发布
    let pending: Vec<(i64,)> =
        sqlx::query_as("SELECT id FROM submissions WHERE plugin_id=$1 AND status='pending'")
            .bind(p.id)
            .fetch_all(&mut *tx)
            .await
            .map_err(anyhow::Error::from)?;
    for (sid,) in &pending {
        let why = format!(
            "插件已被开发者{}",
            if to_status == "paused" {
                "暂停"
            } else {
                "下架"
            }
        );
        sqlx::query("UPDATE submissions SET status='rejected', reviewed_by=$1, reviewed_at=now(), review_reason=$2 WHERE id=$3")
            .bind(user.dev.id)
            .bind(&why)
            .bind(sid)
            .execute(&mut *tx)
            .await
            .map_err(anyhow::Error::from)?;
        crate::audit::write(
            &mut tx,
            &user.dev.developer_id,
            "developer",
            "submission_rejected",
            Some(&p.plugin_id),
            Some(*sid),
            Some("pending"),
            Some("rejected"),
            Some(&why),
        )
        .await?;
    }
    let action = if to_status == "paused" {
        "plugin_paused"
    } else {
        "plugin_unlisted"
    };
    crate::audit::write(
        &mut tx,
        &user.dev.developer_id,
        "developer",
        action,
        Some(&p.plugin_id),
        None,
        Some(p.status.as_str()),
        Some(to_status),
        None,
    )
    .await?;
    tx.commit().await.map_err(anyhow::Error::from)?;
    Ok(Json(json!({ "ok": true, "status": to_status })))
}

/// 恢复（开发者自助）：paused → published，立即重新进入公开索引（无需审核）
async fn resume_plugin_dev(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(plugin_id): Path<String>,
) -> ApiResult<Json<Value>> {
    ensure_not_banned(&user)?;
    let p = load_own_plugin(&state, &user, &plugin_id, false).await?;
    if p.status != "paused" {
        return Err(AppError::Conflict("只有已暂停的插件才能恢复".into()));
    }
    let mut tx = state.pool().begin().await.map_err(anyhow::Error::from)?;
    sqlx::query("UPDATE plugins SET status='published', updated_at=now() WHERE id=$1")
        .bind(p.id)
        .execute(&mut *tx)
        .await
        .map_err(anyhow::Error::from)?;
    crate::audit::write(
        &mut tx,
        &user.dev.developer_id,
        "developer",
        "plugin_resumed",
        Some(&p.plugin_id),
        None,
        Some("paused"),
        Some("published"),
        None,
    )
    .await?;
    tx.commit().await.map_err(anyhow::Error::from)?;
    Ok(Json(json!({ "ok": true, "status": "published" })))
}

/// 开发者概览统计：按状态计数的真实聚合（单次请求）
async fn dev_stats(State(state): State<AppState>, user: CurrentUser) -> ApiResult<Json<Value>> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT status, COUNT(*) FROM plugins WHERE developer_id = $1 GROUP BY status",
    )
    .bind(user.dev.id)
    .fetch_all(state.pool())
    .await
    .map_err(anyhow::Error::from)?;
    let mut by_status = std::collections::BTreeMap::new();
    let mut total = 0i64;
    for (status, count) in rows {
        total += count;
        by_status.insert(status, count);
    }
    let pending_versions: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM submissions s JOIN plugins p ON p.id = s.plugin_id
         WHERE p.developer_id = $1 AND s.status = 'pending'",
    )
    .bind(user.dev.id)
    .fetch_one(state.pool())
    .await
    .map_err(anyhow::Error::from)?;
    let count = |key: &str| by_status.get(key).copied().unwrap_or(0);
    Ok(Json(json!({
        "total": total,
        "published": count("published"),
        "pending": count("pending"),
        "draft": count("draft"),
        "rejected": count("rejected"),
        "stopped": count("stopped"),
        "paused": count("paused"),
        "unlisted": count("unlisted"),
        "cancelled": count("cancelled"),
        "updatesHalted": sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM plugins WHERE developer_id = $1 AND updates_halted AND status = 'published'",
        )
        .bind(user.dev.id)
        .fetch_one(state.pool())
        .await
        .unwrap_or(0),
        "pendingSubmissions": pending_versions.0,
    })))
}
