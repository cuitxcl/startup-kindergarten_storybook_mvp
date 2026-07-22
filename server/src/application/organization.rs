use axum::http::HeaderMap;
use loco_rs::app::AppContext;
use uuid::Uuid;

#[cfg(feature = "db")]
use serde_json::json;

#[cfg(not(feature = "db"))]
use crate::models::WorkspaceRole;
use crate::{
    domains::common,
    error::ApiError,
    models::{
        AuditLogEntry, Classroom, CreateClassroomRequest, CreateMemberRequest, ListQuery,
        PaginationMeta, WorkspaceMember,
    },
};

pub async fn list_members(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    query: ListQuery,
) -> Result<(Vec<WorkspaceMember>, PaginationMeta), ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_admin_db(ctx, headers, workspace_id).await?;
        return crate::repositories::organization::list_members_page(
            &ctx.db,
            workspace_id,
            query.limit,
            query.offset,
        )
        .await
        .map_err(common::db_error);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = ctx
            .shared_store
            .get::<crate::state::SharedState>()
            .ok_or_else(|| ApiError::state_conflict("应用状态未初始化"))?;
        common::require_admin(&state, headers, workspace_id)?;
        let state = state.read().expect("state lock poisoned");
        let members = state
            .members
            .iter()
            .filter(|item| item.workspace_id == workspace_id)
            .cloned()
            .collect();
        Ok(common::paginate_vec(members, query.limit, query.offset))
    }
}

pub async fn create_member(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    payload: CreateMemberRequest,
) -> Result<WorkspaceMember, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_admin_db(ctx, headers, workspace_id).await?;
        if payload.email.trim().is_empty() {
            return Err(ApiError::validation("email", "请输入老师邮箱"));
        }
        let member =
            crate::repositories::organization::create_member(&ctx.db, workspace_id, payload)
                .await
                .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(common::actor_user_id(headers)?),
            "workspace_member.invited",
            "workspace_member",
            Some(member.id),
            json!({
                "email": member.email,
                "role": "school_teacher",
                "classes": member.classes,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(member);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = ctx
            .shared_store
            .get::<crate::state::SharedState>()
            .ok_or_else(|| ApiError::state_conflict("应用状态未初始化"))?;
        common::require_admin(&state, headers, workspace_id)?;
        if payload.email.trim().is_empty() {
            return Err(ApiError::validation("email", "请输入老师邮箱"));
        }
        let mut state = state.write().expect("state lock poisoned");
        let member = WorkspaceMember {
            id: Uuid::new_v4(),
            workspace_id,
            name: value_or(payload.name, "待接受老师"),
            email: payload.email,
            role: WorkspaceRole::SchoolTeacher,
            status: "invited".to_string(),
            classes: common::clean_string_list(payload.classes),
            invitation_token: None,
            invitation_url: None,
        };
        state.members.push(member.clone());
        Ok(member)
    }
}

pub async fn revoke_member_invitation(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    member_id: Uuid,
) -> Result<WorkspaceMember, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_admin_db(ctx, headers, workspace_id).await?;
        let member = crate::repositories::organization::revoke_member_invitation(
            &ctx.db,
            workspace_id,
            member_id,
        )
        .await
        .map_err(common::member_invitation_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(common::actor_user_id(headers)?),
            "workspace_member.invitation_revoked",
            "workspace_member",
            Some(member.id),
            json!({
                "email": member.email,
                "status": member.status,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(member);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = ctx
            .shared_store
            .get::<crate::state::SharedState>()
            .ok_or_else(|| ApiError::state_conflict("应用状态未初始化"))?;
        common::require_admin(&state, headers, workspace_id)?;
        let mut state = state.write().expect("state lock poisoned");
        let member = state
            .members
            .iter_mut()
            .find(|item| item.workspace_id == workspace_id && item.id == member_id)
            .ok_or_else(|| ApiError::not_found("member"))?;
        if member.role != WorkspaceRole::SchoolTeacher || member.status != "invited" {
            return Err(ApiError::state_conflict("只有待接受老师邀请可以撤回"));
        }
        member.status = "revoked".to_string();
        member.invitation_token = None;
        member.invitation_url = None;
        Ok(member.clone())
    }
}

pub async fn list_classrooms(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    query: ListQuery,
) -> Result<(Vec<Classroom>, PaginationMeta), ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_workspace_db(ctx, headers, workspace_id).await?;
        return crate::repositories::organization::list_classrooms_page(
            &ctx.db,
            workspace_id,
            query.limit,
            query.offset,
        )
        .await
        .map_err(common::db_error);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = ctx
            .shared_store
            .get::<crate::state::SharedState>()
            .ok_or_else(|| ApiError::state_conflict("应用状态未初始化"))?;
        common::require_workspace(&state, headers, workspace_id)?;
        let state = state.read().expect("state lock poisoned");
        let classrooms = state
            .classrooms
            .iter()
            .filter(|item| item.workspace_id == workspace_id)
            .cloned()
            .collect();
        Ok(common::paginate_vec(classrooms, query.limit, query.offset))
    }
}

pub async fn create_classroom(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    payload: CreateClassroomRequest,
) -> Result<Classroom, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_admin_db(ctx, headers, workspace_id).await?;
        common::required(payload.name.clone(), "name")?;
        common::required(payload.age_group.clone(), "age_group")?;
        let classroom =
            crate::repositories::organization::create_classroom(&ctx.db, workspace_id, payload)
                .await
                .map_err(common::classroom_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(common::actor_user_id(headers)?),
            "classroom.created",
            "classroom",
            Some(classroom.id),
            json!({
                "name": classroom.name,
                "age_group": classroom.age_group,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(classroom);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = ctx
            .shared_store
            .get::<crate::state::SharedState>()
            .ok_or_else(|| ApiError::state_conflict("应用状态未初始化"))?;
        common::require_admin(&state, headers, workspace_id)?;
        let name = common::required(payload.name, "name")?;
        let age_group = common::required(payload.age_group, "age_group")?;
        let mut state = state.write().expect("state lock poisoned");
        if state
            .classrooms
            .iter()
            .any(|item| item.workspace_id == workspace_id && item.name == name)
        {
            return Err(ApiError::state_conflict("班级名称已存在"));
        }
        let classroom = Classroom {
            id: Uuid::new_v4(),
            workspace_id,
            name,
            age_group,
            teachers: 0,
            children: 0,
            status: "active".to_string(),
        };
        state.classrooms.push(classroom.clone());
        Ok(classroom)
    }
}

pub async fn archive_classroom(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    classroom_id: Uuid,
) -> Result<Classroom, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_admin_db(ctx, headers, workspace_id).await?;
        let classroom = crate::repositories::organization::archive_classroom(
            &ctx.db,
            workspace_id,
            classroom_id,
        )
        .await
        .map_err(common::classroom_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(common::actor_user_id(headers)?),
            "classroom.archived",
            "classroom",
            Some(classroom.id),
            json!({
                "name": classroom.name,
                "status": classroom.status,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(classroom);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = ctx
            .shared_store
            .get::<crate::state::SharedState>()
            .ok_or_else(|| ApiError::state_conflict("应用状态未初始化"))?;
        common::require_admin(&state, headers, workspace_id)?;
        let mut state = state.write().expect("state lock poisoned");
        let classroom = state
            .classrooms
            .iter_mut()
            .find(|item| item.workspace_id == workspace_id && item.id == classroom_id)
            .ok_or_else(|| ApiError::not_found("classroom"))?;
        if classroom.status != "active" {
            return Err(ApiError::state_conflict("只有使用中的班级可以归档"));
        }
        if classroom.children > 0 {
            return Err(ApiError::state_conflict("班级仍有儿童档案，不能归档"));
        }
        classroom.status = "archived".to_string();
        Ok(classroom.clone())
    }
}

pub async fn list_audit_logs(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    query: ListQuery,
) -> Result<(Vec<AuditLogEntry>, PaginationMeta), ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_admin_db(ctx, headers, workspace_id).await?;
        return crate::repositories::audit::list_page_by_workspace(
            &ctx.db,
            workspace_id,
            query.limit,
            query.offset,
        )
        .await
        .map_err(common::db_error);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = ctx
            .shared_store
            .get::<crate::state::SharedState>()
            .ok_or_else(|| ApiError::state_conflict("应用状态未初始化"))?;
        common::require_admin(&state, headers, workspace_id)?;
        Ok(common::paginate_vec(Vec::new(), query.limit, query.offset))
    }
}

#[cfg(not(feature = "db"))]
fn value_or(value: String, fallback: &str) -> String {
    let value = value.trim().to_string();
    if value.is_empty() {
        fallback.to_string()
    } else {
        value
    }
}
