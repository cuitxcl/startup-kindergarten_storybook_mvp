use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::models::{
    Classroom, CreateClassroomRequest, CreateMemberRequest, PaginationMeta,
    WorkspaceInvitationDetail, WorkspaceMember, WorkspaceRole,
};

pub async fn seed_demo_organization(db: &DatabaseConnection) -> Result<(), DbErr> {
    execute(
        db,
        r#"
        insert into classrooms (id, workspace_id, name, age_group, status, created_at, updated_at)
        values
          ('80000000-0000-0000-0000-000000000001', '20000000-0000-0000-0000-000000000001', '小一班', '3-4 岁', 'active', now(), now()),
          ('80000000-0000-0000-0000-000000000002', '20000000-0000-0000-0000-000000000002', '中一班', '4-5 岁', 'active', now(), now())
        on conflict (id) do update
          set name = excluded.name,
              age_group = excluded.age_group,
              status = excluded.status,
              updated_at = now();
        "#,
    )
    .await
}

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

pub async fn list_classrooms_page(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<(Vec<Classroom>, PaginationMeta), DbErr> {
    let limit = limit.unwrap_or(50).clamp(1, 100);
    let offset = offset.unwrap_or(0);
    let total = count_active_classrooms(db, workspace_id).await?;
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select c.id, c.workspace_id, c.name, coalesce(c.age_group, '') as age_group, c.status,
                   coalesce(child_counts.children, 0) as children
            from classrooms c
            left join (
              select classroom_id, count(*)::int as children
              from children
              where workspace_id = $1 and status = 'active' and classroom_id is not null
              group by classroom_id
            ) child_counts on child_counts.classroom_id = c.id
            where c.workspace_id = $1 and c.status = 'active'
            order by c.name
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
            .map(classroom_from_row)
            .collect::<Result<Vec<_>, _>>()?,
        pagination_meta(total, limit, offset),
    ))
}

pub async fn authorized_classrooms_for_user(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<String>, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select classroom_ids
            from workspace_members
            where workspace_id = $1
              and user_id = $2
              and status = 'active'
            limit 1
            "#,
            [workspace_id.into(), user_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("member".to_string()))?;
    let classes: JsonValue = row.try_get("", "classroom_ids")?;
    Ok(classes
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToOwned::to_owned))
                .collect()
        })
        .unwrap_or_default())
}

pub async fn create_classroom(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    payload: CreateClassroomRequest,
) -> Result<Classroom, DbErr> {
    let name = payload.name.trim();
    if name.is_empty() {
        return Err(DbErr::Custom("班级名称不能为空".to_string()));
    }
    let age_group = payload.age_group.trim();
    if age_group.is_empty() {
        return Err(DbErr::Custom("年龄段不能为空".to_string()));
    }
    ensure_classroom_name_available(db, workspace_id, name).await?;
    let id = Uuid::new_v4();
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            insert into classrooms (id, workspace_id, name, age_group, status, created_at, updated_at)
            values ($1, $2, $3, $4, 'active', now(), now())
            returning id, workspace_id, name, coalesce(age_group, '') as age_group, status, 0::int as children
            "#,
            [
                id.into(),
                workspace_id.into(),
                name.into(),
                age_group.into(),
            ],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("classroom".to_string()))?;
    classroom_from_row(row)
}

pub async fn archive_classroom(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    classroom_id: Uuid,
) -> Result<Classroom, DbErr> {
    let classroom = find_classroom_by_id(db, workspace_id, classroom_id).await?;
    if classroom.status != "active" {
        return Err(DbErr::Custom("classroom_not_active".to_string()));
    }
    if classroom.children > 0 {
        return Err(DbErr::Custom("classroom_has_children".to_string()));
    }

    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            update classrooms
            set status = 'archived',
                updated_at = now()
            where id = $1 and workspace_id = $2 and status = 'active'
            returning id, workspace_id, name, coalesce(age_group, '') as age_group, status, 0::int as children
            "#,
            [classroom_id.into(), workspace_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("classroom".to_string()))?;
    classroom_from_row(row)
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

async fn count_active_classrooms(
    db: &DatabaseConnection,
    workspace_id: Uuid,
) -> Result<usize, DbErr> {
    let total: i64 = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "select count(*) as count from classrooms where workspace_id = $1 and status = 'active'",
            [workspace_id.into()],
        ))
        .await?
        .and_then(|row| row.try_get("", "count").ok())
        .unwrap_or(0);
    Ok(total.max(0) as usize)
}

fn pagination_meta(total: usize, limit: usize, offset: usize) -> PaginationMeta {
    PaginationMeta {
        total,
        limit,
        offset: offset.min(total),
        has_more: offset.saturating_add(limit) < total,
    }
}

async fn find_classroom_by_id(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    classroom_id: Uuid,
) -> Result<Classroom, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select c.id, c.workspace_id, c.name, coalesce(c.age_group, '') as age_group, c.status,
                   coalesce(child_counts.children, 0) as children
            from classrooms c
            left join (
              select classroom_id, count(*)::int as children
              from children
              where workspace_id = $1 and status = 'active' and classroom_id is not null
              group by classroom_id
            ) child_counts on child_counts.classroom_id = c.id
            where c.workspace_id = $1 and c.id = $2
            limit 1
            "#,
            [workspace_id.into(), classroom_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("classroom".to_string()))?;
    classroom_from_row(row)
}

async fn ensure_classrooms_exist(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    class_names: &[String],
) -> Result<(), DbErr> {
    if class_names.is_empty() {
        return Ok(());
    }
    let class_names_json =
        serde_json::to_value(class_names).unwrap_or_else(|_| JsonValue::Array(vec![]));
    let found_count: i64 = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select count(*) as count
            from classrooms
            where workspace_id = $1
              and status = 'active'
              and name in (select value from jsonb_array_elements_text($2::jsonb))
            "#,
            [workspace_id.into(), class_names_json.into()],
        ))
        .await?
        .and_then(|row| row.try_get("", "count").ok())
        .unwrap_or(0);

    if found_count as usize == class_names.len() {
        Ok(())
    } else {
        Err(DbErr::RecordNotFound("classroom".to_string()))
    }
}

async fn ensure_classroom_name_available(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    name: &str,
) -> Result<(), DbErr> {
    let exists: bool = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select exists(
              select 1
              from classrooms
              where workspace_id = $1
                and name = $2
            ) as exists
            "#,
            [workspace_id.into(), name.into()],
        ))
        .await?
        .and_then(|row| row.try_get("", "exists").ok())
        .unwrap_or(false);

    if exists {
        Err(DbErr::Custom("classroom_exists".to_string()))
    } else {
        Ok(())
    }
}

fn clean_string_list(items: Vec<String>) -> Vec<String> {
    let mut cleaned = Vec::new();
    for item in items {
        let item = item.trim();
        if item.is_empty() || cleaned.iter().any(|existing| existing == item) {
            continue;
        }
        cleaned.push(item.to_string());
    }
    cleaned
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

fn member_from_row(row: sea_orm::QueryResult) -> Result<WorkspaceMember, DbErr> {
    let classes: JsonValue = row.try_get("", "classroom_ids")?;
    let role: String = row.try_get("", "role")?;
    let id: Uuid = row.try_get("", "id")?;
    let status: String = row.try_get("", "status")?;
    let invitation_token = if status == "invited" {
        Some(id.to_string())
    } else {
        None
    };
    let invitation_url = invitation_token
        .as_ref()
        .map(|token| format!("/invite/{token}"));
    Ok(WorkspaceMember {
        id,
        workspace_id: row.try_get("", "workspace_id")?,
        name: row.try_get("", "name")?,
        email: row.try_get("", "email")?,
        role: parse_workspace_role(&role),
        status,
        classes: classes
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(ToOwned::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
        invitation_token,
        invitation_url,
    })
}

fn invitation_from_row(row: sea_orm::QueryResult) -> Result<WorkspaceInvitationDetail, DbErr> {
    let classes: JsonValue = row.try_get("", "classroom_ids")?;
    let role: String = row.try_get("", "role")?;
    Ok(WorkspaceInvitationDetail {
        token: row.try_get::<Uuid>("", "id")?.to_string(),
        workspace_id: row.try_get("", "workspace_id")?,
        workspace_name: row.try_get("", "workspace_name")?,
        invited_by: "园所管理员".to_string(),
        invited_contact: row.try_get("", "invited_contact")?,
        role: parse_workspace_role(&role),
        classrooms: classes
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(ToOwned::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
        status: row.try_get("", "status")?,
    })
}

fn classroom_from_row(row: sea_orm::QueryResult) -> Result<Classroom, DbErr> {
    let children: i32 = row.try_get("", "children")?;
    Ok(Classroom {
        id: row.try_get("", "id")?,
        workspace_id: row.try_get("", "workspace_id")?,
        name: row.try_get("", "name")?,
        age_group: row.try_get("", "age_group")?,
        teachers: 0,
        children: children.max(0) as u32,
        status: row.try_get("", "status")?,
    })
}

fn parse_workspace_role(value: &str) -> WorkspaceRole {
    match value {
        "school_admin" => WorkspaceRole::SchoolAdmin,
        "school_teacher" => WorkspaceRole::SchoolTeacher,
        "platform_operator" => WorkspaceRole::PlatformOperator,
        _ => WorkspaceRole::PersonalOwner,
    }
}

async fn execute(db: &DatabaseConnection, sql: &str) -> Result<(), DbErr> {
    db.execute(Statement::from_string(DbBackend::Postgres, sql.to_string()))
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_string_list_trims_deduplicates_and_drops_empty_items() {
        let cleaned = clean_string_list(vec![
            " 小一班 ".to_string(),
            "".to_string(),
            "小一班".to_string(),
            "中一班 ".to_string(),
            " ".to_string(),
        ]);

        assert_eq!(cleaned, vec!["小一班".to_string(), "中一班".to_string()]);
    }
}
