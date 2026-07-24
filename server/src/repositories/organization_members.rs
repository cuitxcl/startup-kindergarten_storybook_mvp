use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::{
    models::{
        CreateMemberRequest, PaginationMeta, WorkspaceInvitationDetail, WorkspaceMember,
        WorkspaceRole,
    },
    repositories::{
        organization::{clean_string_list, invitation_from_row, member_from_row, pagination_meta},
        organization_classrooms::ensure_classrooms_exist,
    },
};

pub async fn list_members_page(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<(Vec<WorkspaceMember>, PaginationMeta), DbErr> {
    let limit = limit.unwrap_or(50).clamp(1, 100);
    let offset = offset.unwrap_or(0);
    let total = count_members_by_workspace(db, workspace_id).await?;
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select wm.id, wm.workspace_id, wm.role, wm.status, wm.classroom_ids,
                   u.display_name as name, u.email
            from workspace_members wm
            join users u on u.id = wm.user_id
            where wm.workspace_id = $1
            order by
              case wm.role when 'school_admin' then 0 when 'school_teacher' then 1 else 2 end,
              u.display_name
            limit $2 offset $3
            "#,
            [
                workspace_id.into(),
                (limit as i64).into(),
                (offset as i64).into(),
            ],
        ))
        .await?;
    Ok((
        rows.into_iter()
            .map(member_from_row)
            .collect::<Result<Vec<_>, _>>()?,
        pagination_meta(total, limit, offset),
    ))
}

pub async fn create_member(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    payload: CreateMemberRequest,
) -> Result<WorkspaceMember, DbErr> {
    let email = payload.email.trim().to_lowercase();
    if email.is_empty() {
        return Err(DbErr::Custom("请输入老师邮箱".to_string()));
    }
    let name = if payload.name.trim().is_empty() {
        "待接受老师".to_string()
    } else {
        payload.name.trim().to_string()
    };
    let class_names = clean_string_list(payload.classes);
    ensure_classrooms_exist(db, workspace_id, &class_names).await?;
    let user_id = upsert_invited_user(db, &name, &email).await?;
    let member_id = Uuid::new_v4();
    let classes = serde_json::to_value(&class_names).unwrap_or_else(|_| JsonValue::Array(vec![]));

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        insert into workspace_members
          (id, workspace_id, user_id, role, status, classroom_ids, created_at, updated_at)
        values ($1, $2, $3, 'school_teacher', 'invited', $4, now(), now())
        on conflict (workspace_id, user_id) do update
          set role = 'school_teacher',
              status = 'invited',
              classroom_ids = excluded.classroom_ids,
              updated_at = now()
        "#,
        [
            member_id.into(),
            workspace_id.into(),
            user_id.into(),
            classes.into(),
        ],
    ))
    .await?;

    let mut member = find_member_by_user(db, workspace_id, user_id).await?;
    member.invitation_token = Some(member.id.to_string());
    member.invitation_url = Some(format!("/invite/{}", member.id));
    Ok(member)
}

pub async fn get_invitation(
    db: &DatabaseConnection,
    token: Uuid,
) -> Result<WorkspaceInvitationDetail, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select wm.id, wm.workspace_id, wm.role, wm.status, wm.classroom_ids,
                   u.email as invited_contact,
                   w.name as workspace_name
            from workspace_members wm
            join users u on u.id = wm.user_id
            join workspaces w on w.id = wm.workspace_id
            where wm.id = $1
              and wm.role = 'school_teacher'
              and w.workspace_type = 'school'
            limit 1
            "#,
            [token.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("invitation".to_string()))?;
    invitation_from_row(row)
}

pub async fn accept_invitation(
    db: &DatabaseConnection,
    token: Uuid,
) -> Result<WorkspaceInvitationDetail, DbErr> {
    let invitation = get_invitation(db, token).await?;
    if invitation.status != "invited" {
        return Ok(invitation);
    }

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update workspace_members
        set status = 'active',
            updated_at = now()
        where id = $1 and status = 'invited'
        "#,
        [token.into()],
    ))
    .await?;

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update users
        set status = 'active',
            updated_at = now()
        where id = (
          select user_id
          from workspace_members
          where id = $1
        )
        "#,
        [token.into()],
    ))
    .await?;

    get_invitation(db, token).await
}

pub async fn revoke_member_invitation(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    member_id: Uuid,
) -> Result<WorkspaceMember, DbErr> {
    let member = find_member_by_id(db, workspace_id, member_id).await?;
    if member.role != WorkspaceRole::SchoolTeacher || member.status != "invited" {
        return Err(DbErr::Custom("invitation_not_revocable".to_string()));
    }

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update workspace_members
        set status = 'revoked',
            updated_at = now()
        where id = $1 and workspace_id = $2 and status = 'invited'
        "#,
        [member_id.into(), workspace_id.into()],
    ))
    .await?;

    find_member_by_id(db, workspace_id, member_id).await
}

async fn upsert_invited_user(
    db: &DatabaseConnection,
    name: &str,
    email: &str,
) -> Result<Uuid, DbErr> {
    db.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        insert into users (id, display_name, email, password_hash, status, created_at, updated_at)
        values ($1, $2, $3, null, 'invited', now(), now())
        on conflict (email) do update
          set display_name = excluded.display_name,
              updated_at = now()
        returning id
        "#,
        [Uuid::new_v4().into(), name.into(), email.into()],
    ))
    .await?
    .ok_or_else(|| DbErr::RecordNotFound("user".to_string()))?
    .try_get("", "id")
}

async fn count_members_by_workspace(
    db: &DatabaseConnection,
    workspace_id: Uuid,
) -> Result<usize, DbErr> {
    let total: i64 = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "select count(*) as count from workspace_members where workspace_id = $1",
            [workspace_id.into()],
        ))
        .await?
        .and_then(|row| row.try_get("", "count").ok())
        .unwrap_or(0);
    Ok(total.max(0) as usize)
}

async fn find_member_by_user(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    user_id: Uuid,
) -> Result<WorkspaceMember, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select wm.id, wm.workspace_id, wm.role, wm.status, wm.classroom_ids,
                   u.display_name as name, u.email
            from workspace_members wm
            join users u on u.id = wm.user_id
            where wm.workspace_id = $1 and wm.user_id = $2
            limit 1
            "#,
            [workspace_id.into(), user_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("member".to_string()))?;
    member_from_row(row)
}

async fn find_member_by_id(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    member_id: Uuid,
) -> Result<WorkspaceMember, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select wm.id, wm.workspace_id, wm.role, wm.status, wm.classroom_ids,
                   u.display_name as name, u.email
            from workspace_members wm
            join users u on u.id = wm.user_id
            where wm.workspace_id = $1 and wm.id = $2
            limit 1
            "#,
            [workspace_id.into(), member_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("member".to_string()))?;
    member_from_row(row)
}
