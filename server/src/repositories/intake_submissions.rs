use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::{
    models::{
        ActionResponse, ChildProfile, ConfirmParentIntakeRequest, PaginationMeta, ParentIntake,
        ParentIntakeRequest,
    },
    repositories::{
        children::calculate_completeness,
        intakes::{
            DEFAULT_INTAKE_WORKSPACE_ID, ensure_workspace_exists, intake_from_row,
            optional_trimmed, pagination_meta, resolve_active_link_classroom_id,
            resolve_classroom_id,
        },
    },
};

pub async fn submit_parent_intake(
    db: &DatabaseConnection,
    payload: ParentIntakeRequest,
) -> Result<ActionResponse, DbErr> {
    let interest_count = payload.interests.len();
    let workspace_id = payload.workspace_id.unwrap_or(DEFAULT_INTAKE_WORKSPACE_ID);
    ensure_workspace_exists(db, workspace_id).await?;
    let classroom_id = match payload.link_token.as_deref() {
        Some(token) => resolve_active_link_classroom_id(db, workspace_id, token).await?,
        None => None,
    };
    let interests =
        serde_json::to_value(payload.interests).unwrap_or_else(|_| JsonValue::Array(vec![]));
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        insert into parent_intakes
          (id, workspace_id, classroom_id, child_nickname, age_group, interests, status, confirmed_child_id, created_at, updated_at)
        values ($1, $2, $3, $4, $5, $6, 'submitted', null, now(), now())
        "#,
        [
            Uuid::new_v4().into(),
            workspace_id.into(),
            classroom_id.into(),
            payload.child_nickname.into(),
            payload.age_group.into(),
            interests.into(),
        ],
    ))
    .await?;

    Ok(ActionResponse {
        status: "submitted".to_string(),
        message: format!("资料已提交给老师确认，包含 {interest_count} 个兴趣元素"),
    })
}

async fn count_intakes_by_workspace(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    classroom: Option<&str>,
) -> Result<usize, DbErr> {
    let total: i64 = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select count(*) as count
            from parent_intakes pi
            left join classrooms c on c.id = pi.classroom_id and c.workspace_id = pi.workspace_id
            where pi.workspace_id = $1
              and ($2::text is null or c.name = $2)
            "#,
            [workspace_id.into(), classroom.map(str::to_string).into()],
        ))
        .await?
        .and_then(|row| row.try_get("", "count").ok())
        .unwrap_or(0);
    Ok(total.max(0) as usize)
}

pub async fn list_page_by_workspace(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    classroom: Option<&str>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<(Vec<ParentIntake>, PaginationMeta), DbErr> {
    let limit = limit.unwrap_or(50).clamp(1, 100);
    let offset = offset.unwrap_or(0);
    let classroom_filter = optional_trimmed(classroom);
    let total = count_intakes_by_workspace(db, workspace_id, classroom_filter.as_deref()).await?;
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select pi.id, pi.workspace_id, pi.child_nickname, pi.age_group,
                   c.name as classroom, pi.interests, pi.status,
                   pi.confirmed_child_id, pi.created_at, pi.updated_at
            from parent_intakes pi
            left join classrooms c on c.id = pi.classroom_id and c.workspace_id = pi.workspace_id
            where pi.workspace_id = $1
              and ($4::text is null or c.name = $4)
            order by
              case pi.status when 'submitted' then 0 when 'confirmed' then 1 else 2 end,
              pi.created_at desc
            limit $2 offset $3
            "#,
            [
                workspace_id.into(),
                (limit as i64).into(),
                (offset as i64).into(),
                classroom_filter.into(),
            ],
        ))
        .await?;

    Ok((
        rows.into_iter()
            .map(intake_from_row)
            .collect::<Result<Vec<_>, _>>()?,
        pagination_meta(total, limit, offset),
    ))
}

pub async fn confirm(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    intake_id: Uuid,
    payload: ConfirmParentIntakeRequest,
) -> Result<ChildProfile, DbErr> {
    let intake = find_submitted(db, workspace_id, intake_id).await?;
    let derived = build_child_from_intake(&intake, payload);
    let child_id = Uuid::new_v4();
    let classroom_id = resolve_classroom_id(db, workspace_id, intake.classroom.as_deref()).await?;

    let child_row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            insert into children
              (id, workspace_id, classroom_id, nickname, age_group, interests, traits, focus, completeness, status, created_at, updated_at)
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'active', now(), now())
            returning id, workspace_id, nickname, age_group, interests, traits, focus, completeness, status, updated_at
            "#,
            [
                child_id.into(),
                workspace_id.into(),
                classroom_id.into(),
                derived.nickname.into(),
                derived.age_group.into(),
                derived.interests.into(),
                derived.traits.into(),
                derived.focus.into(),
                derived.completeness.into(),
            ],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("child".to_string()))?;
    let inserted_child_id: Uuid = child_row.try_get("", "id")?;

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update parent_intakes
        set status = 'confirmed',
            confirmed_child_id = $3,
            updated_at = now()
        where workspace_id = $1 and id = $2 and status = 'submitted'
        "#,
        [workspace_id.into(), intake_id.into(), child_id.into()],
    ))
    .await?;

    crate::repositories::children::find(db, workspace_id, inserted_child_id).await
}

pub(crate) fn build_child_from_intake(
    intake: &ParentIntake,
    payload: ConfirmParentIntakeRequest,
) -> DerivedChild {
    let focus = payload
        .focus
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "家长提交资料，待老师补充关注点".to_string());
    let completeness = calculate_completeness(
        &intake.child_nickname,
        &intake.age_group,
        &intake.interests,
        &payload.traits,
        &focus,
    );
    DerivedChild {
        nickname: intake.child_nickname.clone(),
        age_group: intake.age_group.clone(),
        interests: serde_json::to_value(&intake.interests)
            .unwrap_or_else(|_| JsonValue::Array(vec![])),
        traits: serde_json::to_value(&payload.traits).unwrap_or_else(|_| JsonValue::Array(vec![])),
        focus,
        completeness,
    }
}

pub(crate) struct DerivedChild {
    pub nickname: String,
    pub age_group: String,
    pub interests: JsonValue,
    pub traits: JsonValue,
    pub focus: String,
    pub completeness: i32,
}

async fn find_submitted(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    intake_id: Uuid,
) -> Result<ParentIntake, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select pi.id, pi.workspace_id, pi.child_nickname, pi.age_group,
                   c.name as classroom, pi.interests, pi.status,
                   pi.confirmed_child_id, pi.created_at, pi.updated_at
            from parent_intakes pi
            left join classrooms c on c.id = pi.classroom_id and c.workspace_id = pi.workspace_id
            where pi.workspace_id = $1 and pi.id = $2 and pi.status = 'submitted'
            limit 1
            "#,
            [workspace_id.into(), intake_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("parent_intake".to_string()))?;

    intake_from_row(row)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_intake() -> ParentIntake {
        ParentIntake {
            id: Uuid::new_v4(),
            workspace_id: Uuid::new_v4(),
            child_nickname: "乐乐".to_string(),
            age_group: "4-5 岁".to_string(),
            classroom: None,
            interests: vec!["积木车".to_string(), "唱歌".to_string()],
            status: "submitted".to_string(),
            confirmed_child_id: None,
            created_at: "2026-07-19 10:00".to_string(),
            updated_at: "2026-07-19 10:00".to_string(),
        }
    }

    #[test]
    fn build_child_from_intake_uses_intake_data() {
        let intake = sample_intake();
        let derived = build_child_from_intake(
            &intake,
            ConfirmParentIntakeRequest {
                focus: Some("午睡适应".to_string()),
                traits: vec!["慢热".to_string(), "喜欢鼓励".to_string()],
            },
        );

        assert_eq!(derived.nickname, "乐乐");
        assert_eq!(derived.age_group, "4-5 岁");
        assert_eq!(derived.focus, "午睡适应");
        assert_eq!(
            derived.completeness,
            calculate_completeness(
                &intake.child_nickname,
                &intake.age_group,
                &intake.interests,
                &["慢热".to_string(), "喜欢鼓励".to_string()],
                "午睡适应",
            )
        );
    }

    #[test]
    fn build_child_from_intake_falls_back_to_default_focus() {
        let intake = sample_intake();
        let derived = build_child_from_intake(
            &intake,
            ConfirmParentIntakeRequest {
                focus: Some("   ".to_string()),
                traits: vec![],
            },
        );

        assert_eq!(derived.focus, "家长提交资料，待老师补充关注点");
    }
}
