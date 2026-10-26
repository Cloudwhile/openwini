
#[derive(Deserialize)]
struct OfficialBody {
    #[serde(rename = "pluginId")]
    plugin_id: String,
    #[serde(rename = "repositoryUrl")]
    repository_url: String,
    version: String,
    description: String,
    #[serde(rename = "homepageUrl")]
    homepage_url: Option<String>,
    #[serde(rename = "docsUrl")]
    docs_url: Option<String>,
}

/// 官方插件：管理员直接创建并发布，不走申请队列，但记审计
async fn official_create(
    State(state): State<AppState>,
    admin: AdminUser,
    Json(body): Json<OfficialBody>,
) -> ApiResult<Json<Value>> {
    let mut tx = state.pool().begin().await.map_err(anyhow::Error::from)?;
    official_upsert(&state, &mut tx, &admin, None, &body, true).await?;
    tx.commit().await.map_err(anyhow::Error::from)?;
    Ok(Json(json!({ "ok": true, "status": "published" })))
}

async fn official_new_version(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(plugin_id): Path<String>,
    Json(body): Json<OfficialBody>,
) -> ApiResult<Json<Value>> {
    let mut tx = state.pool().begin().await.map_err(anyhow::Error::from)?;
    official_upsert(&state, &mut tx, &admin, Some(&plugin_id), &body, false).await?;
    tx.commit().await.map_err(anyhow::Error::from)?;
    Ok(Json(json!({ "ok": true, "status": "published" })))
}

async fn official_upsert(
    state: &AppState,
    tx: &mut PgConnection,
    admin: &AdminUser,
    existing_plugin_id: Option<&str>,
    body: &OfficialBody,
    is_create: bool,
) -> ApiResult<()> {
    let token = admin.0.github_token()?;
    let official_row: (i64,) = sqlx::query_as("SELECT id FROM developers WHERE developer_id=$1")
        .bind(v::OFFICIAL_DEVELOPER_ID)
        .fetch_one(&mut *tx)
        .await
        .map_err(anyhow::Error::from)?;

    let (plugin_row_id, plugin_id) = match existing_plugin_id {
        Some(pid) => {
            let (rid, rpid, rdev): (i64, String, i64) = sqlx::query_as(
                "SELECT id, plugin_id, developer_id FROM plugins WHERE plugin_id=$1",
            )
            .bind(pid)
            .fetch_optional(&mut *tx)
            .await
            .map_err(anyhow::Error::from)?
            .ok_or_else(|| AppError::NotFound("插件不存在".into()))?;
            if rdev != official_row.0 {
                return Err(AppError::Conflict(
                    "该插件不属于直发账号（winiofficial），请走审核流程".into(),
                ));
            }
            (rid, rpid)
        }
        None => {
            let pid = body.plugin_id.trim().to_string();
            if !v::valid_plugin_id(&pid) {
                return Err(AppError::BadRequest("插件标识格式不合法".into()));
            }
            if v::is_reserved_plugin_id(&pid) {
                return Err(AppError::Conflict("该插件标识是内置保留标识".into()));
            }
            let taken: Option<(i64,)> = sqlx::query_as("SELECT id FROM plugins WHERE plugin_id=$1")
                .bind(&pid)
                .fetch_optional(&mut *tx)
                .await
                .map_err(anyhow::Error::from)?;
            if taken.is_some() {
                return Err(AppError::Conflict(format!("插件标识 {pid} 已被使用")));
            }
            (0, pid)
        }
    };

    // 同一开发者的已发布插件必须同仓库（winiofficial 同样受源契约约束）
    let other_repo: Option<(String,)> = sqlx::query_as(
        "SELECT DISTINCT repository_url FROM plugins
         WHERE developer_id=$1 AND status='published' AND repository_url <> $2 LIMIT 1",
    )
    .bind(official_row.0)
    .bind(&body.repository_url)
    .fetch_optional(&mut *tx)
    .await
    .map_err(anyhow::Error::from)?;
    if let Some((other,)) = other_repo {
        return Err(AppError::Conflict(format!(
            "直发账号已有使用仓库 {other} 的已发布插件（源契约要求同开发者同仓库）"
        )));
    }

    let known = sub::known_permission_set(state)
        .await
        .map_err(anyhow::Error::from)?;
    let validated = sub::validate_full(
        state,
        &token,
        &plugin_id,
        body.repository_url.trim(),
        body.version.trim(),
        None, /* 直发：描述以清单为准 */
        None, /* 英文描述同样以清单 description_en 为准（无则为空） */
        &known,
    )
    .await
    .map_err(|diffs| {
        AppError::BadRequest(json!({ "error": "清单比对未通过", "differences": diffs }).to_string())
    })?;

    let entry = v::CatalogEntry {
        id: plugin_id.clone(),
        name: validated.manifest_name.clone(),
        version: validated.version.clone(),
        developer_id: v::OFFICIAL_DEVELOPER_ID.to_string(),
        repository_url: body.repository_url.trim().to_string(),
        download_url: validated.download_url.clone(),
        sha256: validated.sha256.clone(),
        min_host_version: validated.min_host_version.clone(),
        contract_version: validated.contract_version,
        permissions: validated.permissions.clone(),
        description: validated.description.clone(),
    };
    entry
        .validate(&known)
        .map_err(|e| AppError::BadRequest(format!("契约校验未通过：{e}")))?;
    check_catalog_quotas(tx, official_row.0, !is_create).await?;

    let final_plugin_row = if is_create {
        let row: (i64,) = sqlx::query_as(
            "INSERT INTO plugins (developer_id, plugin_id, repository_url, homepage_url, docs_url, status)
             VALUES ($1,$2,$3,$4,$5,'published') RETURNING id",
        )
        .bind(official_row.0)
        .bind(&plugin_id)
        .bind(body.repository_url.trim())
        .bind(body.homepage_url.as_deref().map(str::trim).filter(|s| !s.is_empty()))
        .bind(body.docs_url.as_deref().map(str::trim).filter(|s| !s.is_empty()))
        .fetch_one(&mut *tx)
        .await
        .map_err(anyhow::Error::from)?;
        row.0
    } else {
        sqlx::query("UPDATE plugins SET repository_url=$1, homepage_url=$2, docs_url=$3, updated_at=now() WHERE id=$4")
            .bind(body.repository_url.trim())
            .bind(body.homepage_url.as_deref().map(str::trim).filter(|s| !s.is_empty()))
            .bind(body.docs_url.as_deref().map(str::trim).filter(|s| !s.is_empty()))
            .bind(plugin_row_id)
            .execute(&mut *tx)
            .await
            .map_err(anyhow::Error::from)?;
        plugin_row_id
    };
    let sub_id: (i64,) = sqlx::query_as(
        "INSERT INTO submissions
            (plugin_id, version, manifest_json, manifest_name, description, description_en, permissions,
             download_url, sha256, min_host_version, contract_version, status, is_new_version,
             review_reason, submitted_by, reviewed_by, reviewed_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,'approved',$12,'直发发布',$13,$14,now())
         RETURNING id",
    )
    .bind(final_plugin_row)
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
    .bind(!is_create)
    .bind(admin.0.dev.id)
    .bind(admin.0.dev.id)
    .fetch_one(&mut *tx)
    .await
    .map_err(anyhow::Error::from)?;
    sqlx::query("UPDATE plugins SET current_version=$1, current_submission_id=$2, updated_at=now() WHERE id=$3")
        .bind(&validated.version)
        .bind(sub_id.0)
        .bind(final_plugin_row)
        .execute(&mut *tx)
        .await
        .map_err(anyhow::Error::from)?;
    crate::audit::write(
        tx,
        &admin.0.dev.developer_id,
        "admin",
        if is_create {
            "official_publish"
        } else {
            "official_version_published"
        },
        Some(&plugin_id),
        Some(sub_id.0),
        None,
        Some("published"),
        Some("直发账号（winiofficial）直接发布，未走申请队列"),
    )
    .await?;
    Ok(())
}

// —— 开发者管理 ——
