use axum::http::HeaderMap;
use loco_rs::app::AppContext;
use uuid::Uuid;

#[cfg(feature = "db")]
use serde_json::json;

use crate::{
    domains::common,
    error::ApiError,
    models::{ChildProfile, CreateChildRequest, ListQuery, PaginationMeta, UpdateChildRequest},
};

pub async fn list(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    query: ListQuery,
) -> Result<(Vec<ChildProfile>, PaginationMeta), ApiError> {
    #[cfg(feature = "db")]
    {
        let workspace = common::require_workspace_db(ctx, headers, workspace_id).await?;
        let scope = common::child_classroom_scope(ctx, headers, workspace_id, &workspace).await?;
        let result = match scope {
            Some(classrooms) => {
                crate::repositories::children::list_page_by_workspace_for_classrooms(
                    &ctx.db,
                    workspace_id,
                    &classrooms,
                    query.limit,
                    query.offset,
                )
                .await
                .map_err(common::db_error)?
            }
            None => crate::repositories::children::list_page_by_workspace(
                &ctx.db,
                workspace_id,
                query.limit,
                query.offset,
            )
            .await
            .map_err(common::db_error)?,
        };
        return Ok(result);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = ctx
            .shared_store
            .get::<crate::state::SharedState>()
            .ok_or_else(|| ApiError::state_conflict("应用状态未初始化"))?;
        common::require_workspace(&state, headers, workspace_id)?;
        let state = state.read().expect("state lock poisoned");
        let children = state
            .children
            .iter()
            .filter(|item| item.workspace_id == workspace_id)
            .cloned()
            .collect();
        Ok(common::paginate_vec(children, query.limit, query.offset))
    }
}

pub async fn create(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    payload: CreateChildRequest,
) -> Result<ChildProfile, ApiError> {
    #[cfg(feature = "db")]
    {
        let workspace = common::require_editor_db(ctx, headers, workspace_id).await?;
        let scope = common::child_classroom_scope(ctx, headers, workspace_id, &workspace).await?;
        common::ensure_child_classroom_allowed(scope.as_deref(), payload.classroom.as_deref())?;
        let nickname = common::required(payload.nickname, "nickname")?;
        let age_group = common::required(payload.age_group, "age_group")?;
        let focus = common::required(payload.focus, "focus")?;
        let child = crate::repositories::children::create(
            &ctx.db,
            workspace_id,
            CreateChildRequest {
                nickname,
                age_group,
                classroom: payload.classroom,
                interests: common::clean_string_list(payload.interests),
                traits: common::clean_string_list(payload.traits),
                focus,
            },
        )
        .await
        .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(common::actor_user_id(headers)?),
            "child.created",
            "child",
            Some(child.id),
            json!({
                "nickname": child.nickname,
                "classroom": child.classroom,
                "completeness": child.completeness,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(child);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = ctx
            .shared_store
            .get::<crate::state::SharedState>()
            .ok_or_else(|| ApiError::state_conflict("应用状态未初始化"))?;
        common::require_editor(&state, headers, workspace_id)?;
        let mut state = state.write().expect("state lock poisoned");
        let child = ChildProfile {
            id: Uuid::new_v4(),
            workspace_id,
            nickname: common::required(payload.nickname, "nickname")?,
            age_group: common::required(payload.age_group, "age_group")?,
            classroom: payload.classroom,
            interests: common::clean_string_list(payload.interests),
            traits: common::clean_string_list(payload.traits),
            focus: common::required(payload.focus, "focus")?,
            completeness: 80,
            status: "active".to_string(),
            updated_at: "刚刚".to_string(),
        };
        state.children.push(child.clone());
        Ok(child)
    }
}

pub async fn get(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    child_id: Uuid,
) -> Result<ChildProfile, ApiError> {
    #[cfg(feature = "db")]
    {
        let workspace = common::require_workspace_db(ctx, headers, workspace_id).await?;
        let scope = common::child_classroom_scope(ctx, headers, workspace_id, &workspace).await?;
        let child = match scope {
            Some(classrooms) => crate::repositories::children::find_for_classrooms(
                &ctx.db,
                workspace_id,
                child_id,
                &classrooms,
            )
            .await
            .map_err(common::db_error)?,
            None => crate::repositories::children::find(&ctx.db, workspace_id, child_id)
                .await
                .map_err(common::db_error)?,
        };
        return Ok(child);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = ctx
            .shared_store
            .get::<crate::state::SharedState>()
            .ok_or_else(|| ApiError::state_conflict("应用状态未初始化"))?;
        common::require_workspace(&state, headers, workspace_id)?;
        state
            .read()
            .expect("state lock poisoned")
            .children
            .iter()
            .find(|item| item.workspace_id == workspace_id && item.id == child_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("child"))
    }
}

pub async fn update(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    child_id: Uuid,
    payload: UpdateChildRequest,
) -> Result<ChildProfile, ApiError> {
    #[cfg(feature = "db")]
    {
        let workspace = common::require_editor_db(ctx, headers, workspace_id).await?;
        let scope = common::child_classroom_scope(ctx, headers, workspace_id, &workspace).await?;
        if let Some(classrooms) = scope.as_deref() {
            crate::repositories::children::find_for_classrooms(
                &ctx.db,
                workspace_id,
                child_id,
                classrooms,
            )
            .await
            .map_err(common::db_error)?;
        }
        common::ensure_child_classroom_allowed(scope.as_deref(), payload.classroom.as_deref())?;
        let payload = UpdateChildRequest {
            nickname: match payload.nickname {
                Some(value) => Some(common::required(value, "nickname")?),
                None => None,
            },
            age_group: match payload.age_group {
                Some(value) => Some(common::required(value, "age_group")?),
                None => None,
            },
            classroom: payload.classroom,
            interests: payload.interests.map(common::clean_string_list),
            traits: payload.traits.map(common::clean_string_list),
            focus: match payload.focus {
                Some(value) => Some(common::required(value, "focus")?),
                None => None,
            },
        };
        let child = crate::repositories::children::update(&ctx.db, workspace_id, child_id, payload)
            .await
            .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(common::actor_user_id(headers)?),
            "child.updated",
            "child",
            Some(child.id),
            json!({
                "nickname": child.nickname,
                "classroom": child.classroom,
                "completeness": child.completeness,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(child);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = ctx
            .shared_store
            .get::<crate::state::SharedState>()
            .ok_or_else(|| ApiError::state_conflict("应用状态未初始化"))?;
        common::require_editor(&state, headers, workspace_id)?;
        let mut state = state.write().expect("state lock poisoned");
        let child = state
            .children
            .iter_mut()
            .find(|item| item.workspace_id == workspace_id && item.id == child_id)
            .ok_or_else(|| ApiError::not_found("child"))?;
        if let Some(value) = payload.nickname {
            child.nickname = common::required(value, "nickname")?;
        }
        if let Some(value) = payload.age_group {
            child.age_group = common::required(value, "age_group")?;
        }
        if payload.classroom.is_some() {
            child.classroom = payload.classroom;
        }
        if let Some(value) = payload.interests {
            child.interests = common::clean_string_list(value);
        }
        if let Some(value) = payload.traits {
            child.traits = common::clean_string_list(value);
        }
        if let Some(value) = payload.focus {
            child.focus = common::required(value, "focus")?;
        }
        child.updated_at = "刚刚".to_string();
        Ok(child.clone())
    }
}

pub async fn archive(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    child_id: Uuid,
) -> Result<ChildProfile, ApiError> {
    #[cfg(feature = "db")]
    {
        let workspace = common::require_editor_db(ctx, headers, workspace_id).await?;
        let scope = common::child_classroom_scope(ctx, headers, workspace_id, &workspace).await?;
        if let Some(classrooms) = scope.as_deref() {
            crate::repositories::children::find_for_classrooms(
                &ctx.db,
                workspace_id,
                child_id,
                classrooms,
            )
            .await
            .map_err(common::db_error)?;
        }
        let child = crate::repositories::children::archive(&ctx.db, workspace_id, child_id)
            .await
            .map_err(common::child_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(common::actor_user_id(headers)?),
            "child.archived",
            "child",
            Some(child.id),
            json!({
                "nickname": child.nickname,
                "classroom": child.classroom,
                "status": child.status,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(child);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = ctx
            .shared_store
            .get::<crate::state::SharedState>()
            .ok_or_else(|| ApiError::state_conflict("应用状态未初始化"))?;
        common::require_editor(&state, headers, workspace_id)?;
        let mut state = state.write().expect("state lock poisoned");
        let child = state
            .children
            .iter_mut()
            .find(|item| item.workspace_id == workspace_id && item.id == child_id)
            .ok_or_else(|| ApiError::not_found("child"))?;
        if child.status != "active" {
            return Err(ApiError::state_conflict("只有使用中的儿童档案可以归档"));
        }
        child.status = "archived".to_string();
        child.updated_at = "刚刚".to_string();
        Ok(child.clone())
    }
}

pub async fn restore(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    child_id: Uuid,
) -> Result<ChildProfile, ApiError> {
    #[cfg(feature = "db")]
    {
        let workspace = common::require_editor_db(ctx, headers, workspace_id).await?;
        let scope = common::child_classroom_scope(ctx, headers, workspace_id, &workspace).await?;
        if let Some(classrooms) = scope.as_deref() {
            crate::repositories::children::find_any_status_for_classrooms(
                &ctx.db,
                workspace_id,
                child_id,
                classrooms,
            )
            .await
            .map_err(common::db_error)?;
        }
        let child = crate::repositories::children::restore(&ctx.db, workspace_id, child_id)
            .await
            .map_err(common::child_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(common::actor_user_id(headers)?),
            "child.restored",
            "child",
            Some(child.id),
            json!({
                "nickname": child.nickname,
                "classroom": child.classroom,
                "status": child.status,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(child);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = ctx
            .shared_store
            .get::<crate::state::SharedState>()
            .ok_or_else(|| ApiError::state_conflict("应用状态未初始化"))?;
        common::require_editor(&state, headers, workspace_id)?;
        let mut state = state.write().expect("state lock poisoned");
        let child = state
            .children
            .iter_mut()
            .find(|item| item.workspace_id == workspace_id && item.id == child_id)
            .ok_or_else(|| ApiError::not_found("child"))?;
        if child.status != "archived" {
            return Err(ApiError::state_conflict("只有已归档的儿童档案可以恢复"));
        }
        child.status = "active".to_string();
        child.updated_at = "刚刚".to_string();
        Ok(child.clone())
    }
}
