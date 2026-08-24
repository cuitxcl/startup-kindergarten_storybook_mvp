use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement, TransactionTrait};
use uuid::Uuid;

use crate::models::{
    StorybookAssetReference, StorybookAssetSummary, StorybookVisualReferenceSummary,
};

pub const MAX_CREATION_ASSET_REFERENCES: u32 = 5;
pub const DEFAULT_ASSET_VISIBILITY_SCOPE: &str = "creation_session";
pub const DEFAULT_ASSET_RETENTION_POLICY: &str = "session_scoped";

pub struct CreateAssetReferenceInput {
    pub workspace_id: Uuid,
    pub session_id: Uuid,
    pub uploaded_by: Uuid,
    pub storage_key: String,
    pub original_filename: String,
    pub content_type: String,
    pub byte_size: i64,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub kind: String,
    pub idempotency_key: Option<String>,
}

pub struct UpdateAssetReferenceInput {
    pub kind: Option<String>,
    pub display_name: Option<String>,
    pub usage: Option<String>,
    pub material_id: Option<String>,
}

pub async fn count_effective_references(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    session_id: Uuid,
) -> Result<u32, DbErr> {
    let count: i64 = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select count(*) as count
            from storybook_asset_references
            where workspace_id = $1
              and creation_session_id = $2
              and status not in ('unused', 'revoked')
            "#,
            [workspace_id.into(), session_id.into()],
        ))
        .await?
        .and_then(|row| row.try_get("", "count").ok())
        .unwrap_or(0);
    Ok(count.max(0) as u32)
}

pub async fn remaining_slots(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    session_id: Uuid,
) -> Result<u32, DbErr> {
    let used = count_effective_references(db, workspace_id, session_id).await?;
    Ok(MAX_CREATION_ASSET_REFERENCES.saturating_sub(used))
}

pub async fn create_asset_reference(
    db: &DatabaseConnection,
    input: CreateAssetReferenceInput,
) -> Result<StorybookAssetReference, DbErr> {
    if let Some(idempotency_key) = input.idempotency_key.as_deref() {
        if let Some(existing) =
            find_by_idempotency_key(db, input.workspace_id, input.session_id, idempotency_key)
                .await?
        {
            return Ok(existing);
        }
    }

    let asset_id = Uuid::new_v4();
    let reference_id = Uuid::new_v4();
    let txn = db.begin().await?;
    txn.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        insert into storybook_assets
          (id, workspace_id, uploaded_by, storage_key, original_filename, content_type, byte_size,
           width, height, status, visibility_scope, retention_policy, created_at, updated_at)
        values ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'ready', $10, $11, now(), now())
        "#,
        [
            asset_id.into(),
            input.workspace_id.into(),
            input.uploaded_by.into(),
            input.storage_key.into(),
            input.original_filename.into(),
            input.content_type.into(),
            input.byte_size.into(),
            input.width.into(),
            input.height.into(),
            DEFAULT_ASSET_VISIBILITY_SCOPE.into(),
            DEFAULT_ASSET_RETENTION_POLICY.into(),
        ],
    ))
    .await?;
    txn.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        insert into storybook_asset_references
          (id, workspace_id, creation_session_id, asset_id, kind, status, idempotency_key,
           created_at, updated_at)
        values ($1, $2, $3, $4, $5, 'awaiting_usage', $6, now(), now())
        "#,
        [
            reference_id.into(),
            input.workspace_id.into(),
            input.session_id.into(),
            asset_id.into(),
            input.kind.into(),
            input.idempotency_key.into(),
        ],
    ))
    .await?;
    txn.commit().await?;
    find(db, input.workspace_id, input.session_id, reference_id).await
}

pub async fn list_for_session(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    session_id: Uuid,
) -> Result<Vec<StorybookAssetReference>, DbErr> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            select_asset_reference_sql("r.workspace_id = $1 and r.creation_session_id = $2")
                .as_str(),
            [workspace_id.into(), session_id.into()],
        ))
        .await?;
    rows.into_iter().map(row_to_asset_reference).collect()
}

pub async fn list_by_ids(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    asset_reference_ids: &[Uuid],
) -> Result<Vec<StorybookAssetReference>, DbErr> {
    if asset_reference_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            select_asset_reference_sql("r.workspace_id = $1 and r.id = any($2)").as_str(),
            [workspace_id.into(), asset_reference_ids.to_vec().into()],
        ))
        .await?;
    rows.into_iter().map(row_to_asset_reference).collect()
}

pub async fn find(
    db: &impl ConnectionTrait,
    workspace_id: Uuid,
    session_id: Uuid,
    asset_reference_id: Uuid,
) -> Result<StorybookAssetReference, DbErr> {
    db.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        select_asset_reference_sql(
            "r.workspace_id = $1 and r.creation_session_id = $2 and r.id = $3",
        )
        .as_str(),
        [
            workspace_id.into(),
            session_id.into(),
            asset_reference_id.into(),
        ],
    ))
    .await?
    .ok_or_else(|| DbErr::RecordNotFound("storybook_asset_reference".to_string()))
    .and_then(row_to_asset_reference)
}

pub async fn update_reference(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    session_id: Uuid,
    asset_reference_id: Uuid,
    input: UpdateAssetReferenceInput,
) -> Result<StorybookAssetReference, DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_asset_references
        set kind = coalesce($4, kind),
            display_name = coalesce($5, display_name),
            usage = coalesce($6, usage),
            material_id = coalesce($7, material_id),
            status = case
                when revoked_at is not null then 'revoked'
                when coalesce($6, usage) is null then 'awaiting_usage'
                when coalesce($6, usage) = 'unused' then 'unused'
                when coalesce($6, usage) = 'name_only' then 'ready'
                else case (
                    select v.status
                    from storybook_visual_references v
                    where v.workspace_id = storybook_asset_references.workspace_id
                      and v.asset_reference_id = storybook_asset_references.id
                      and v.is_active = true
                    limit 1
                )
                    when 'confirmed' then 'ready'
                    when 'awaiting_confirmation' then 'awaiting_confirmation'
                    when 'failed' then 'failed'
                    else 'awaiting_reference'
                end
            end,
            updated_at = now()
        where workspace_id = $1 and creation_session_id = $2 and id = $3
        "#,
        [
            workspace_id.into(),
            session_id.into(),
            asset_reference_id.into(),
            input.kind.into(),
            input.display_name.into(),
            input.usage.into(),
            input.material_id.into(),
        ],
    ))
    .await?;
    find(db, workspace_id, session_id, asset_reference_id).await
}

pub async fn create_visual_reference(
    db: &impl ConnectionTrait,
    workspace_id: Uuid,
    session_id: Uuid,
    asset_reference_id: Uuid,
    generation_job_id: Option<Uuid>,
    idempotency_key: Option<String>,
) -> Result<StorybookAssetReference, DbErr> {
    if let Some(idempotency_key) = idempotency_key.as_deref() {
        if has_visual_reference_idempotency_key(
            db,
            workspace_id,
            asset_reference_id,
            idempotency_key,
        )
        .await?
        {
            sync_reference_status(db, workspace_id, session_id, asset_reference_id).await?;
            return find(db, workspace_id, session_id, asset_reference_id).await;
        }
    }

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_visual_references
        set is_active = false,
            status = case when status in ('queued', 'generating', 'awaiting_confirmation') then 'rejected' else status end,
            updated_at = now()
        where workspace_id = $1 and asset_reference_id = $2 and is_active = true
        "#,
        [workspace_id.into(), asset_reference_id.into()],
    ))
    .await?;

    let visual_reference_id = Uuid::new_v4();
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        insert into storybook_visual_references
          (id, workspace_id, asset_reference_id, generation_job_id, status, idempotency_key,
           is_active, created_at, updated_at)
        values ($1, $2, $3, $4, 'queued', $5, true, now(), now())
        "#,
        [
            visual_reference_id.into(),
            workspace_id.into(),
            asset_reference_id.into(),
            generation_job_id.into(),
            idempotency_key.into(),
        ],
    ))
    .await?;
    sync_reference_status(db, workspace_id, session_id, asset_reference_id).await?;
    find(db, workspace_id, session_id, asset_reference_id).await
}

pub async fn mark_visual_reference_awaiting_confirmation(
    db: &impl ConnectionTrait,
    workspace_id: Uuid,
    session_id: Uuid,
    visual_reference_id: Uuid,
    image_storage_key: String,
) -> Result<StorybookAssetReference, DbErr> {
    let asset_reference_id =
        asset_reference_id_for_visual_reference(db, workspace_id, session_id, visual_reference_id)
            .await?;
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_visual_references
        set status = 'awaiting_confirmation',
            image_storage_key = $3,
            failure_reason = null,
            updated_at = now()
        where workspace_id = $1 and id = $2 and is_active = true
        "#,
        [
            workspace_id.into(),
            visual_reference_id.into(),
            image_storage_key.into(),
        ],
    ))
    .await?;
    sync_reference_status(db, workspace_id, session_id, asset_reference_id).await?;
    find(db, workspace_id, session_id, asset_reference_id).await
}

pub async fn mark_visual_reference_awaiting_confirmation_by_job(
    db: &impl ConnectionTrait,
    workspace_id: Uuid,
    generation_job_id: Uuid,
    image_storage_key: String,
) -> Result<(), DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select r.creation_session_id, v.id as visual_reference_id
            from storybook_visual_references v
            join storybook_asset_references r
              on r.id = v.asset_reference_id and r.workspace_id = v.workspace_id
            where v.workspace_id = $1
              and v.generation_job_id = $2
              and v.is_active = true
            limit 1
            "#,
            [workspace_id.into(), generation_job_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("storybook_visual_reference".to_string()))?;
    let session_id: Uuid = row.try_get("", "creation_session_id")?;
    let visual_reference_id: Uuid = row.try_get("", "visual_reference_id")?;
    mark_visual_reference_awaiting_confirmation(
        db,
        workspace_id,
        session_id,
        visual_reference_id,
        image_storage_key,
    )
    .await
    .map(|_| ())
}

pub async fn mark_visual_reference_generating_by_job(
    db: &impl ConnectionTrait,
    workspace_id: Uuid,
    generation_job_id: Uuid,
) -> Result<(), DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select r.creation_session_id, r.id as asset_reference_id
            from storybook_visual_references v
            join storybook_asset_references r
              on r.id = v.asset_reference_id and r.workspace_id = v.workspace_id
            where v.workspace_id = $1
              and v.generation_job_id = $2
              and v.is_active = true
            limit 1
            "#,
            [workspace_id.into(), generation_job_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("storybook_visual_reference".to_string()))?;
    let session_id: Uuid = row.try_get("", "creation_session_id")?;
    let asset_reference_id: Uuid = row.try_get("", "asset_reference_id")?;
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_visual_references
        set status = 'generating',
            updated_at = now()
        where workspace_id = $1 and generation_job_id = $2 and is_active = true
        "#,
        [workspace_id.into(), generation_job_id.into()],
    ))
    .await?;
    sync_reference_status(db, workspace_id, session_id, asset_reference_id).await
}

pub async fn confirm_visual_reference(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    session_id: Uuid,
    visual_reference_id: Uuid,
    confirmed_by: Uuid,
) -> Result<StorybookAssetReference, DbErr> {
    let asset_reference_id =
        asset_reference_id_for_visual_reference(db, workspace_id, session_id, visual_reference_id)
            .await?;
    let current_status: String = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select status
            from storybook_visual_references
            where workspace_id = $1 and id = $2 and is_active = true
            limit 1
            "#,
            [workspace_id.into(), visual_reference_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("storybook_visual_reference".to_string()))?
        .try_get("", "status")?;
    if current_status == "confirmed" {
        sync_reference_status(db, workspace_id, session_id, asset_reference_id).await?;
        return find(db, workspace_id, session_id, asset_reference_id).await;
    }
    if current_status != "awaiting_confirmation" {
        return Err(DbErr::Custom(
            "visual_reference_not_ready_for_confirmation".to_string(),
        ));
    }
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_visual_references
        set status = 'confirmed',
            confirmed_at = now(),
            confirmed_by = $3,
            updated_at = now()
        where workspace_id = $1 and id = $2 and is_active = true
        "#,
        [
            workspace_id.into(),
            visual_reference_id.into(),
            confirmed_by.into(),
        ],
    ))
    .await?;
    sync_reference_status(db, workspace_id, session_id, asset_reference_id).await?;
    find(db, workspace_id, session_id, asset_reference_id).await
}

pub async fn mark_visual_reference_failed(
    db: &impl ConnectionTrait,
    workspace_id: Uuid,
    session_id: Uuid,
    visual_reference_id: Uuid,
    failure_reason: String,
) -> Result<StorybookAssetReference, DbErr> {
    let asset_reference_id =
        asset_reference_id_for_visual_reference(db, workspace_id, session_id, visual_reference_id)
            .await?;
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_visual_references
        set status = 'failed',
            failure_reason = $3,
            updated_at = now()
        where workspace_id = $1 and id = $2 and is_active = true
        "#,
        [
            workspace_id.into(),
            visual_reference_id.into(),
            failure_reason.into(),
        ],
    ))
    .await?;
    sync_reference_status(db, workspace_id, session_id, asset_reference_id).await?;
    find(db, workspace_id, session_id, asset_reference_id).await
}

pub async fn mark_visual_reference_failed_by_job(
    db: &impl ConnectionTrait,
    workspace_id: Uuid,
    generation_job_id: Uuid,
    failure_reason: String,
) -> Result<(), DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select r.creation_session_id, v.id as visual_reference_id
            from storybook_visual_references v
            join storybook_asset_references r
              on r.id = v.asset_reference_id and r.workspace_id = v.workspace_id
            where v.workspace_id = $1
              and v.generation_job_id = $2
              and v.is_active = true
            limit 1
            "#,
            [workspace_id.into(), generation_job_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("storybook_visual_reference".to_string()))?;
    let session_id: Uuid = row.try_get("", "creation_session_id")?;
    let visual_reference_id: Uuid = row.try_get("", "visual_reference_id")?;
    mark_visual_reference_failed(
        db,
        workspace_id,
        session_id,
        visual_reference_id,
        failure_reason,
    )
    .await
    .map(|_| ())
}

pub async fn revoke_reference(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    session_id: Uuid,
    asset_reference_id: Uuid,
    revoked_by: Uuid,
) -> Result<StorybookAssetReference, DbErr> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_asset_references
        set status = 'revoked',
            revoked_at = now(),
            revoked_by = $4,
            updated_at = now()
        where workspace_id = $1 and creation_session_id = $2 and id = $3
        "#,
        [
            workspace_id.into(),
            session_id.into(),
            asset_reference_id.into(),
            revoked_by.into(),
        ],
    ))
    .await?;
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_visual_references
        set is_active = false,
            status = case when status in ('queued', 'generating', 'awaiting_confirmation') then 'rejected' else status end,
            updated_at = now()
        where workspace_id = $1 and asset_reference_id = $2 and is_active = true
        "#,
        [workspace_id.into(), asset_reference_id.into()],
    ))
    .await?;
    find(db, workspace_id, session_id, asset_reference_id).await
}

pub async fn blocking_references_for_generation(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    session_id: Uuid,
) -> Result<Vec<StorybookAssetReference>, DbErr> {
    let references = list_for_session(db, workspace_id, session_id).await?;
    Ok(references
        .into_iter()
        .filter(|reference| {
            matches!(
                reference.status.as_str(),
                "awaiting_usage" | "awaiting_reference" | "awaiting_confirmation" | "failed"
            )
        })
        .collect())
}

pub async fn confirmed_references_for_generation(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    session_id: Uuid,
) -> Result<Vec<StorybookAssetReference>, DbErr> {
    let references = list_for_session(db, workspace_id, session_id).await?;
    Ok(references
        .into_iter()
        .filter(|reference| reference.status == "ready")
        .collect())
}

fn select_asset_reference_sql(where_clause: &str) -> String {
    format!(
        r#"
        select
          r.id,
          r.asset_id,
          r.kind,
          r.display_name,
          r.usage,
          r.status,
          r.material_id,
          r.revoked_at,
          r.revoked_by,
          r.created_at,
          r.updated_at,
          r.workspace_id,
          r.creation_session_id,
          a.storage_key,
          a.status as asset_status,
          a.processing_message,
          a.content_type,
          a.byte_size,
          a.width,
          a.height,
          a.visibility_scope,
          a.retention_policy,
          v.id as visual_reference_id,
          v.status as visual_reference_status,
          v.generation_job_id,
          v.image_storage_key,
          v.failure_reason,
          v.confirmed_at,
          v.confirmed_by
        from storybook_asset_references r
        join storybook_assets a on a.id = r.asset_id and a.workspace_id = r.workspace_id
        left join storybook_visual_references v
          on v.asset_reference_id = r.id and v.workspace_id = r.workspace_id and v.is_active = true
        where {where_clause}
        order by r.created_at asc
        "#
    )
}

fn row_to_asset_reference(row: sea_orm::QueryResult) -> Result<StorybookAssetReference, DbErr> {
    let visual_reference_id: Option<Uuid> = row.try_get("", "visual_reference_id")?;
    let workspace_id: Uuid = row.try_get("", "workspace_id")?;
    let session_id: Uuid = row.try_get("", "creation_session_id")?;
    let asset_id: Uuid = row.try_get("", "asset_id")?;
    let generation_job_id: Option<Uuid> = row.try_get("", "generation_job_id")?;
    let visual_reference = match visual_reference_id {
        Some(id) => Some(StorybookVisualReferenceSummary {
            id,
            status: row.try_get("", "visual_reference_status")?,
            generation_job_id,
            preview_url: row
                .try_get::<Option<String>>("", "image_storage_key")?
                .and(generation_job_id)
                .map(|job_id| {
                    format!("/api/workspaces/{workspace_id}/generation-jobs/{job_id}/image")
                }),
            failure_reason: row.try_get("", "failure_reason")?,
            confirmed_at: row.try_get("", "confirmed_at")?,
            confirmed_by: row.try_get("", "confirmed_by")?,
        }),
        None => None,
    };

    Ok(StorybookAssetReference {
        id: row.try_get("", "id")?,
        asset_id,
        asset: StorybookAssetSummary {
            id: asset_id,
            storage_key: row.try_get("", "storage_key")?,
            status: row.try_get("", "asset_status")?,
            processing_message: row.try_get("", "processing_message")?,
            content_type: row.try_get("", "content_type")?,
            byte_size: row.try_get("", "byte_size")?,
            width: row.try_get("", "width")?,
            height: row.try_get("", "height")?,
            visibility_scope: row.try_get("", "visibility_scope")?,
            retention_policy: row.try_get("", "retention_policy")?,
        },
        kind: row.try_get("", "kind")?,
        display_name: row.try_get("", "display_name")?,
        usage: row.try_get("", "usage")?,
        status: row.try_get("", "status")?,
        material_id: row.try_get("", "material_id")?,
        preview_url: Some(format!(
            "/api/workspaces/{}/storybook-creation-sessions/{}/assets/{}/preview",
            workspace_id, session_id, asset_id
        )),
        visual_reference,
        revoked_at: row.try_get::<Option<DateTime<Utc>>>("", "revoked_at")?,
        revoked_by: row.try_get("", "revoked_by")?,
        created_at: row.try_get("", "created_at")?,
        updated_at: row.try_get("", "updated_at")?,
    })
}

pub async fn find_by_idempotency_key(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    session_id: Uuid,
    idempotency_key: &str,
) -> Result<Option<StorybookAssetReference>, DbErr> {
    db.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        select_asset_reference_sql(
            "r.workspace_id = $1 and r.creation_session_id = $2 and r.idempotency_key = $3",
        )
        .as_str(),
        [
            workspace_id.into(),
            session_id.into(),
            idempotency_key.into(),
        ],
    ))
    .await?
    .map(row_to_asset_reference)
    .transpose()
}

pub async fn find_by_visual_reference_idempotency_key(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    session_id: Uuid,
    asset_reference_id: Uuid,
    idempotency_key: &str,
) -> Result<Option<StorybookAssetReference>, DbErr> {
    db.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        select_asset_reference_sql(
            "r.workspace_id = $1 and r.creation_session_id = $2 and r.id = $3 and v.idempotency_key = $4",
        )
        .as_str(),
        [
            workspace_id.into(),
            session_id.into(),
            asset_reference_id.into(),
            idempotency_key.into(),
        ],
    ))
    .await?
    .map(row_to_asset_reference)
    .transpose()
}

pub async fn has_visual_reference_idempotency_key(
    db: &impl ConnectionTrait,
    workspace_id: Uuid,
    asset_reference_id: Uuid,
    idempotency_key: &str,
) -> Result<bool, DbErr> {
    let count: i64 = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select count(*) as count
            from storybook_visual_references
            where workspace_id = $1 and asset_reference_id = $2 and idempotency_key = $3
            "#,
            [
                workspace_id.into(),
                asset_reference_id.into(),
                idempotency_key.into(),
            ],
        ))
        .await?
        .and_then(|row| row.try_get("", "count").ok())
        .unwrap_or(0);
    Ok(count > 0)
}

async fn asset_reference_id_for_visual_reference(
    db: &impl ConnectionTrait,
    workspace_id: Uuid,
    session_id: Uuid,
    visual_reference_id: Uuid,
) -> Result<Uuid, DbErr> {
    db.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        select r.id
        from storybook_visual_references v
        join storybook_asset_references r
          on r.id = v.asset_reference_id and r.workspace_id = v.workspace_id
        where v.workspace_id = $1
          and r.creation_session_id = $2
          and v.id = $3
        "#,
        [
            workspace_id.into(),
            session_id.into(),
            visual_reference_id.into(),
        ],
    ))
    .await?
    .ok_or_else(|| DbErr::RecordNotFound("storybook_visual_reference".to_string()))?
    .try_get("", "id")
}

async fn sync_reference_status(
    db: &impl ConnectionTrait,
    workspace_id: Uuid,
    session_id: Uuid,
    asset_reference_id: Uuid,
) -> Result<(), DbErr> {
    let usage: Option<String> = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select usage
            from storybook_asset_references
            where workspace_id = $1 and creation_session_id = $2 and id = $3
            "#,
            [
                workspace_id.into(),
                session_id.into(),
                asset_reference_id.into(),
            ],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("storybook_asset_reference".to_string()))?
        .try_get("", "usage")?;

    let visual_status: Option<String> = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select status
            from storybook_visual_references
            where workspace_id = $1 and asset_reference_id = $2 and is_active = true
            "#,
            [workspace_id.into(), asset_reference_id.into()],
        ))
        .await?
        .and_then(|row| row.try_get("", "status").ok());
    let next_status = aggregate_status_for_usage(usage.as_deref(), visual_status.as_deref(), false);
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_asset_references
        set status = $4,
            updated_at = now()
        where workspace_id = $1 and creation_session_id = $2 and id = $3
        "#,
        [
            workspace_id.into(),
            session_id.into(),
            asset_reference_id.into(),
            next_status.into(),
        ],
    ))
    .await?;
    Ok(())
}

fn aggregate_status_for_usage(
    usage: Option<&str>,
    visual_status: Option<&str>,
    revoked: bool,
) -> &'static str {
    if revoked {
        return "revoked";
    }
    match usage {
        None => "awaiting_usage",
        Some("unused") => "unused",
        Some("name_only") => "ready",
        Some(_) => match visual_status {
            Some("confirmed") => "ready",
            Some("awaiting_confirmation") => "awaiting_confirmation",
            Some("failed") => "failed",
            Some("queued") | Some("generating") => "awaiting_reference",
            _ => "awaiting_reference",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::aggregate_status_for_usage;

    #[test]
    fn aggregate_status_matches_asset_reference_state_machine() {
        assert_eq!(
            aggregate_status_for_usage(None, None, false),
            "awaiting_usage"
        );
        assert_eq!(
            aggregate_status_for_usage(Some("unused"), None, false),
            "unused"
        );
        assert_eq!(
            aggregate_status_for_usage(Some("name_only"), None, false),
            "ready"
        );
        assert_eq!(
            aggregate_status_for_usage(Some("main_character"), None, false),
            "awaiting_reference"
        );
        assert_eq!(
            aggregate_status_for_usage(Some("main_character"), Some("queued"), false),
            "awaiting_reference"
        );
        assert_eq!(
            aggregate_status_for_usage(Some("main_character"), Some("generating"), false),
            "awaiting_reference"
        );
        assert_eq!(
            aggregate_status_for_usage(
                Some("main_character"),
                Some("awaiting_confirmation"),
                false
            ),
            "awaiting_confirmation"
        );
        assert_eq!(
            aggregate_status_for_usage(Some("main_character"), Some("confirmed"), false),
            "ready"
        );
        assert_eq!(
            aggregate_status_for_usage(Some("main_character"), Some("failed"), false),
            "failed"
        );
        assert_eq!(
            aggregate_status_for_usage(Some("main_character"), Some("confirmed"), true),
            "revoked"
        );
    }
}
