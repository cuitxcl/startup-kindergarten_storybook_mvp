use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use serde_json::Value as JsonValue;
use uuid::Uuid;

pub use super::organization_classrooms::{
    archive_classroom, authorized_classrooms_for_user, create_classroom, list_classrooms_page,
};
pub use super::organization_members::{
    accept_invitation, create_member, get_invitation, list_members_page, revoke_member_invitation,
};

use crate::models::{
    Classroom, PaginationMeta, WorkspaceInvitationDetail, WorkspaceMember, WorkspaceRole,
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

pub(crate) fn pagination_meta(total: usize, limit: usize, offset: usize) -> PaginationMeta {
    PaginationMeta {
        total,
        limit,
        offset: offset.min(total),
        has_more: offset.saturating_add(limit) < total,
    }
}

pub(crate) fn clean_string_list(items: Vec<String>) -> Vec<String> {
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

pub(crate) fn member_from_row(row: sea_orm::QueryResult) -> Result<WorkspaceMember, DbErr> {
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

pub(crate) fn invitation_from_row(
    row: sea_orm::QueryResult,
) -> Result<WorkspaceInvitationDetail, DbErr> {
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

pub(crate) fn classroom_from_row(row: sea_orm::QueryResult) -> Result<Classroom, DbErr> {
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

pub(crate) fn parse_workspace_role(value: &str) -> WorkspaceRole {
    match value {
        "school_admin" => WorkspaceRole::SchoolAdmin,
        "school_teacher" => WorkspaceRole::SchoolTeacher,
        "platform_operator" => WorkspaceRole::PlatformOperator,
        _ => WorkspaceRole::PersonalOwner,
    }
}

pub(crate) async fn execute(db: &DatabaseConnection, sql: &str) -> Result<(), DbErr> {
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
