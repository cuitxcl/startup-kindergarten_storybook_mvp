use axum::http::HeaderMap;
use loco_rs::app::AppContext;
use uuid::Uuid;

#[cfg(feature = "db")]
use serde_json::json;

use crate::{
    domains::common,
    error::ApiError,
    models::{
        ActionResponse, ChildProfile, ConfirmParentIntakeRequest, CreateParentIntakeLinkRequest,
        ParentIntake, ParentIntakeLink, ParentIntakeLinkBulkActionQuery, ParentIntakeLinkListQuery,
        ParentIntakeListQuery, ParentIntakeRequest, PublicParentIntakeLink,
    },
};

pub async fn list_intakes(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    query: ParentIntakeListQuery,
) -> Result<(Vec<ParentIntake>, crate::models::PaginationMeta), ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_admin_db(ctx, headers, workspace_id).await?;
        return crate::repositories::intakes::list_page_by_workspace(
            &ctx.db,
            workspace_id,
            query.classroom.as_deref(),
            query.limit,
            query.offset,
        )
        .await
        .map_err(common::db_error);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_admin(&state, headers, workspace_id)?;
        let intakes = vec![ParentIntake {
            id: Uuid::new_v4(),
            workspace_id,
            child_nickname: "家长提交样例".to_string(),
            age_group: "4-5 岁".to_string(),
            interests: vec!["画画".to_string(), "小汽车".to_string()],
            classroom: query.classroom.clone(),
            status: "submitted".to_string(),
            confirmed_child_id: None,
            created_at: "刚刚".to_string(),
            updated_at: "刚刚".to_string(),
        }];
        Ok(common::paginate_vec(intakes, query.limit, query.offset))
    }
}

pub async fn list_links(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    query: ParentIntakeLinkListQuery,
) -> Result<(Vec<ParentIntakeLink>, crate::models::PaginationMeta), ApiError> {
    validate_link_status(query.status.as_deref())?;
    #[cfg(feature = "db")]
    {
        common::require_admin_db(ctx, headers, workspace_id).await?;
        return crate::repositories::intakes::list_links_page(
            &ctx.db,
            workspace_id,
            query.status.as_deref(),
            query.classroom.as_deref(),
            query.limit,
            query.offset,
        )
        .await
        .map_err(common::db_error);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_admin(&state, headers, workspace_id)?;
        let links = vec![ParentIntakeLink {
            id: Uuid::new_v4(),
            workspace_id,
            token: "demo-token".to_string(),
            label: "演示家长资料链接".to_string(),
            classroom: query.classroom.clone(),
            status: "active".to_string(),
            url: "/link/intake/demo-token".to_string(),
            expires_at: None,
            access_count: 0,
            last_accessed_at: None,
            created_at: "刚刚".to_string(),
            updated_at: "刚刚".to_string(),
        }];
        let filtered = links
            .into_iter()
            .filter(|link| {
                query
                    .status
                    .as_deref()
                    .is_none_or(|status| link.status == status)
            })
            .collect();
        Ok(common::paginate_vec(filtered, query.limit, query.offset))
    }
}

pub async fn create_link(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    payload: CreateParentIntakeLinkRequest,
) -> Result<ParentIntakeLink, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_admin_db(ctx, headers, workspace_id).await?;
        let actor_id = common::actor_user_id(headers)?;
        let link =
            crate::repositories::intakes::create_link(&ctx.db, workspace_id, actor_id, payload)
                .await
                .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(actor_id),
            "parent_intake_link.created",
            "parent_intake_link",
            Some(link.id),
            json!({
                "label": link.label,
                "status": link.status,
                "expires_at": link.expires_at,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(link);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_admin(&state, headers, workspace_id)?;
        Ok(ParentIntakeLink {
            id: Uuid::new_v4(),
            workspace_id,
            token: "demo-token".to_string(),
            label: payload
                .label
                .unwrap_or_else(|| "演示家长资料链接".to_string()),
            classroom: payload.classroom,
            status: "active".to_string(),
            url: "/link/intake/demo-token".to_string(),
            expires_at: None,
            access_count: 0,
            last_accessed_at: None,
            created_at: "刚刚".to_string(),
            updated_at: "刚刚".to_string(),
        })
    }
}

pub async fn revoke_link(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    link_id: Uuid,
) -> Result<ParentIntakeLink, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_admin_db(ctx, headers, workspace_id).await?;
        let actor_id = common::actor_user_id(headers)?;
        let link = crate::repositories::intakes::revoke_link(&ctx.db, workspace_id, link_id)
            .await
            .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(actor_id),
            "parent_intake_link.revoked",
            "parent_intake_link",
            Some(link.id),
            json!({
                "label": link.label,
                "status": link.status,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(link);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_admin(&state, headers, workspace_id)?;
        Ok(ParentIntakeLink {
            id: link_id,
            workspace_id,
            token: "demo-token".to_string(),
            label: "演示家长资料链接".to_string(),
            classroom: None,
            status: "revoked".to_string(),
            url: "/link/intake/demo-token".to_string(),
            expires_at: None,
            access_count: 0,
            last_accessed_at: None,
            created_at: "刚刚".to_string(),
            updated_at: "刚刚".to_string(),
        })
    }
}

pub async fn revoke_active_links(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    query: ParentIntakeLinkBulkActionQuery,
) -> Result<ActionResponse, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_admin_db(ctx, headers, workspace_id).await?;
        let actor_id = common::actor_user_id(headers)?;
        let revoked_count = crate::repositories::intakes::revoke_active_links(
            &ctx.db,
            workspace_id,
            query.classroom.as_deref(),
        )
        .await
        .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(actor_id),
            "parent_intake_link.active_revoked",
            "parent_intake_link",
            None,
            json!({
                "revoked_count": revoked_count,
                "classroom": query.classroom,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(ActionResponse {
            status: "revoked".to_string(),
            message: format!("已停用 {revoked_count} 条可填写家长资料链接"),
        });
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_admin(&state, headers, workspace_id)?;
        Ok(ActionResponse {
            status: "revoked".to_string(),
            message: "已停用 1 条可填写家长资料链接".to_string(),
        })
    }
}

pub async fn get_public_link(
    ctx: &AppContext,
    token: String,
) -> Result<PublicParentIntakeLink, ApiError> {
    #[cfg(feature = "db")]
    {
        let token = token.trim();
        if token.is_empty() {
            return Err(ApiError::validation("token", "家长资料链接 token 不能为空"));
        }
        return crate::repositories::intakes::get_public_link(&ctx.db, token)
            .await
            .map_err(common::db_error);
    }

    #[cfg(not(feature = "db"))]
    {
        Ok(PublicParentIntakeLink {
            token,
            workspace_id: crate::repositories::intakes::DEFAULT_INTAKE_WORKSPACE_ID,
            workspace_name: "星星幼儿园".to_string(),
            label: "演示家长资料链接".to_string(),
            classroom: None,
            status: "active".to_string(),
            expires_at: None,
        })
    }
}

pub async fn confirm(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    intake_id: Uuid,
    payload: ConfirmParentIntakeRequest,
) -> Result<ChildProfile, ApiError> {
    #[cfg(feature = "db")]
    {
        common::require_admin_db(ctx, headers, workspace_id).await?;
        let child =
            crate::repositories::intakes::confirm(&ctx.db, workspace_id, intake_id, payload)
                .await
                .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            Some(common::actor_user_id(headers)?),
            "parent_intake.confirmed",
            "parent_intake",
            Some(intake_id),
            json!({
                "confirmed_child_id": child.id,
                "child_nickname": child.nickname,
                "completeness": child.completeness,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(child);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_admin(&state, headers, workspace_id)?;
        Ok(ChildProfile {
            id: Uuid::new_v4(),
            workspace_id,
            nickname: format!("家长提交 {}", &intake_id.to_string()[..8]),
            age_group: "4-5 岁".to_string(),
            classroom: None,
            interests: vec!["画画".to_string(), "小汽车".to_string()],
            traits: payload.traits,
            focus: payload
                .focus
                .unwrap_or_else(|| "家长提交资料，待老师补充关注点".to_string()),
            completeness: 72,
            status: "active".to_string(),
            updated_at: "刚刚".to_string(),
        })
    }
}

pub async fn submit(
    ctx: &AppContext,
    payload: ParentIntakeRequest,
) -> Result<ActionResponse, ApiError> {
    #[cfg(feature = "db")]
    {
        let child_nickname = common::required(payload.child_nickname, "child_nickname")?;
        let age_group = common::required(payload.age_group, "age_group")?;
        let link_token = payload.link_token.clone();
        let interest_count = payload.interests.len();
        let workspace_id =
            resolve_parent_intake_workspace(ctx, payload.workspace_id, payload.link_token).await?;
        let response = crate::repositories::intakes::submit_parent_intake(
            &ctx.db,
            ParentIntakeRequest {
                workspace_id: Some(workspace_id),
                link_token: link_token.clone(),
                child_nickname: child_nickname.clone(),
                age_group: age_group.clone(),
                interests: payload.interests,
            },
        )
        .await
        .map_err(common::db_error)?;
        crate::repositories::audit::log(
            &ctx.db,
            Some(workspace_id),
            None,
            "parent_intake.submitted",
            "parent_intake",
            None,
            json!({
                "child_nickname": child_nickname,
                "age_group": age_group,
                "interest_count": interest_count,
                "link_token": link_token,
            }),
        )
        .await
        .map_err(common::db_error)?;
        return Ok(response);
    }

    #[cfg(not(feature = "db"))]
    {
        common::required(payload.child_nickname, "child_nickname")?;
        common::required(payload.age_group, "age_group")?;
        resolve_parent_intake_workspace(ctx, payload.workspace_id, payload.link_token).await?;
        let interest_count = payload.interests.len();
        Ok(ActionResponse {
            status: "submitted".to_string(),
            message: format!("资料已提交给老师确认，包含 {interest_count} 个兴趣元素"),
        })
    }
}

fn validate_link_status(status: Option<&str>) -> Result<(), ApiError> {
    match status.map(str::trim).filter(|value| !value.is_empty()) {
        None | Some("active" | "revoked" | "expired") => Ok(()),
        Some(_) => Err(ApiError::validation(
            "status",
            "状态只能是 active、revoked 或 expired",
        )),
    }
}

async fn resolve_parent_intake_workspace(
    ctx: &AppContext,
    workspace_id: Option<Uuid>,
    link_token: Option<String>,
) -> Result<Uuid, ApiError> {
    let Some(raw_token) = link_token else {
        return workspace_id
            .ok_or_else(|| ApiError::validation("link_token", "家长资料链接缺少目标空间 token"));
    };
    let token = raw_token.trim();
    if token.is_empty() {
        return Err(ApiError::validation(
            "link_token",
            "家长资料链接 token 不能为空",
        ));
    }

    let token_workspace_id = resolve_parent_intake_token(ctx, token).await?;
    if let Some(payload_workspace_id) = workspace_id {
        if payload_workspace_id != token_workspace_id {
            return Err(ApiError::validation(
                "workspace_id",
                "家长资料链接与目标空间不匹配",
            ));
        }
    }
    Ok(token_workspace_id)
}

#[cfg(feature = "db")]
async fn resolve_parent_intake_token(ctx: &AppContext, token: &str) -> Result<Uuid, ApiError> {
    match crate::repositories::intakes::resolve_link_workspace(&ctx.db, token).await {
        Ok(workspace_id) => Ok(workspace_id),
        Err(sea_orm::DbErr::RecordNotFound(_)) if token == "demo-token" => {
            Ok(crate::repositories::intakes::DEFAULT_INTAKE_WORKSPACE_ID)
        }
        Err(sea_orm::DbErr::RecordNotFound(_)) => Err(ApiError::not_found("parent_intake_link")),
        Err(err) => Err(common::db_error(err)),
    }
}

#[cfg(not(feature = "db"))]
async fn resolve_parent_intake_token(_ctx: &AppContext, token: &str) -> Result<Uuid, ApiError> {
    if token == "demo-token" {
        return Ok(crate::repositories::intakes::DEFAULT_INTAKE_WORKSPACE_ID);
    }
    if let Some(value) = token.strip_prefix("workspace-") {
        return Uuid::parse_str(value)
            .map_err(|_| ApiError::validation("link_token", "家长资料链接 token 无效"));
    }
    Err(ApiError::not_found("parent_intake_link"))
}

#[cfg(not(feature = "db"))]
fn shared_state(ctx: &AppContext) -> Result<crate::state::SharedState, ApiError> {
    ctx.shared_store
        .get::<crate::state::SharedState>()
        .ok_or_else(|| ApiError::state_conflict("应用状态未初始化"))
}
