use axum::http::HeaderMap;
#[cfg(feature = "db")]
use loco_rs::app::AppContext;
use uuid::Uuid;

use crate::{error::ApiError, models::*};

#[cfg(not(feature = "db"))]
use crate::state::SharedState;

#[cfg(feature = "db")]
pub fn db_error(err: sea_orm::DbErr) -> ApiError {
    match err {
        sea_orm::DbErr::RecordNotFound(resource) if resource == "user" => ApiError::unauthorized(),
        sea_orm::DbErr::RecordNotFound(resource) if resource == "workspace" => {
            ApiError::not_found("workspace")
        }
        sea_orm::DbErr::RecordNotFound(resource) if resource == "child" => {
            ApiError::not_found("child")
        }
        sea_orm::DbErr::RecordNotFound(resource) if resource == "parent_intake" => {
            ApiError::not_found("parent_intake")
        }
        sea_orm::DbErr::RecordNotFound(resource) if resource == "parent_intake_link" => {
            ApiError::not_found("parent_intake_link")
        }
        sea_orm::DbErr::RecordNotFound(resource) if resource == "invitation" => {
            ApiError::not_found("invitation")
        }
        sea_orm::DbErr::RecordNotFound(resource) if resource == "storybook" => {
            ApiError::not_found("storybook")
        }
        sea_orm::DbErr::RecordNotFound(resource) if resource == "page" => {
            ApiError::not_found("page")
        }
        sea_orm::DbErr::RecordNotFound(resource) if resource == "share_link" => {
            ApiError::not_found("share_link")
        }
        sea_orm::DbErr::RecordNotFound(resource) if resource == "export_job" => {
            ApiError::not_found("export_job")
        }
        sea_orm::DbErr::RecordNotFound(resource) if resource == "template" => {
            ApiError::not_found("template")
        }
        sea_orm::DbErr::RecordNotFound(resource) if resource == "submission" => {
            ApiError::not_found("submission")
        }
        sea_orm::DbErr::RecordNotFound(resource) if resource == "member" => {
            ApiError::not_found("member")
        }
        sea_orm::DbErr::RecordNotFound(resource) if resource == "classroom" => {
            ApiError::not_found("classroom")
        }
        sea_orm::DbErr::RecordNotFound(resource) if resource == "generation_job" => {
            ApiError::not_found("generation_job")
        }
        sea_orm::DbErr::RecordNotFound(_) => ApiError::not_found("resource"),
        other => ApiError::state_conflict(format!("数据库操作失败：{other}")),
    }
}

#[cfg(feature = "db")]
pub fn child_error(err: sea_orm::DbErr) -> ApiError {
    match err {
        sea_orm::DbErr::Custom(message) if message == "child_not_active" => {
            ApiError::state_conflict("只有使用中的儿童档案可以归档")
        }
        sea_orm::DbErr::Custom(message) if message == "child_not_archived" => {
            ApiError::state_conflict("只有已归档的儿童档案可以恢复")
        }
        other => db_error(other),
    }
}

#[cfg(feature = "db")]
pub fn classroom_error(err: sea_orm::DbErr) -> ApiError {
    match err {
        sea_orm::DbErr::Custom(message) if message == "classroom_exists" => {
            ApiError::state_conflict("班级名称已存在")
        }
        sea_orm::DbErr::Custom(message) if message == "年龄段不能为空" => {
            ApiError::validation("age_group", message)
        }
        sea_orm::DbErr::Custom(message) if message == "班级名称不能为空" => {
            ApiError::validation("name", message)
        }
        sea_orm::DbErr::Custom(message) if message == "classroom_has_children" => {
            ApiError::state_conflict("班级仍有儿童档案，不能归档")
        }
        sea_orm::DbErr::Custom(message) if message == "classroom_not_active" => {
            ApiError::state_conflict("只有使用中的班级可以归档")
        }
        other => db_error(other),
    }
}

#[cfg(feature = "db")]
pub fn member_invitation_error(err: sea_orm::DbErr) -> ApiError {
    match err {
        sea_orm::DbErr::Custom(message) if message == "invitation_not_revocable" => {
            ApiError::state_conflict("只有待接受老师邀请可以撤回")
        }
        other => db_error(other),
    }
}

pub fn bearer_token(headers: &HeaderMap) -> Result<&str, ApiError> {
    headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or_else(ApiError::unauthorized)
}

pub fn dev_token_user_id(headers: &HeaderMap) -> Result<Option<Uuid>, ApiError> {
    let token = bearer_token(headers)?;
    crate::repositories::auth::user_id_from_token(token).ok_or_else(ApiError::unauthorized)
}

#[cfg(feature = "db")]
pub fn actor_user_id(headers: &HeaderMap) -> Result<Uuid, ApiError> {
    Ok(dev_token_user_id(headers)?.unwrap_or(crate::repositories::auth::DEMO_USER_ID))
}

#[cfg(not(feature = "db"))]
pub fn require_login(state: &SharedState, headers: &HeaderMap) -> Result<(), ApiError> {
    let token = bearer_token(headers)?;
    if token != state.read().expect("state lock poisoned").token {
        return Err(ApiError::unauthorized());
    }
    Ok(())
}

#[cfg(not(feature = "db"))]
pub fn require_workspace(
    state: &SharedState,
    headers: &HeaderMap,
    workspace_id: Uuid,
) -> Result<Workspace, ApiError> {
    require_login(state, headers)?;
    state
        .read()
        .expect("state lock poisoned")
        .workspaces
        .iter()
        .find(|workspace| workspace.id == workspace_id)
        .cloned()
        .ok_or_else(|| ApiError::forbidden("无权访问该空间"))
}

#[cfg(not(feature = "db"))]
pub fn require_editor(
    state: &SharedState,
    headers: &HeaderMap,
    workspace_id: Uuid,
) -> Result<Workspace, ApiError> {
    let workspace = require_workspace(state, headers, workspace_id)?;
    match workspace.role {
        WorkspaceRole::PersonalOwner
        | WorkspaceRole::SchoolAdmin
        | WorkspaceRole::SchoolTeacher => Ok(workspace),
        WorkspaceRole::PlatformOperator => Err(ApiError::forbidden("平台运营员不能编辑空间内容")),
    }
}

#[cfg(not(feature = "db"))]
pub fn require_admin(
    state: &SharedState,
    headers: &HeaderMap,
    workspace_id: Uuid,
) -> Result<Workspace, ApiError> {
    let workspace = require_workspace(state, headers, workspace_id)?;
    if workspace.role == WorkspaceRole::SchoolAdmin {
        Ok(workspace)
    } else {
        Err(ApiError::forbidden("需要园所管理员权限"))
    }
}

#[cfg(feature = "db")]
pub async fn require_workspace_db(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
) -> Result<Workspace, ApiError> {
    let user_id = dev_token_user_id(headers)?;
    crate::repositories::auth::list_current_workspaces(&ctx.db, user_id)
        .await
        .map_err(db_error)?
        .into_iter()
        .find(|workspace| workspace.id == workspace_id)
        .ok_or_else(|| ApiError::forbidden("无权访问该空间"))
}

#[cfg(feature = "db")]
pub async fn require_editor_db(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
) -> Result<Workspace, ApiError> {
    let workspace = require_workspace_db(ctx, headers, workspace_id).await?;
    match workspace.role {
        WorkspaceRole::PersonalOwner
        | WorkspaceRole::SchoolAdmin
        | WorkspaceRole::SchoolTeacher => Ok(workspace),
        WorkspaceRole::PlatformOperator => Err(ApiError::forbidden("平台运营员不能编辑空间内容")),
    }
}

#[cfg(feature = "db")]
pub async fn require_admin_db(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
) -> Result<Workspace, ApiError> {
    let workspace = require_workspace_db(ctx, headers, workspace_id).await?;
    if workspace.role == WorkspaceRole::SchoolAdmin {
        Ok(workspace)
    } else {
        Err(ApiError::forbidden("需要园所管理员权限"))
    }
}

#[cfg(feature = "db")]
pub async fn require_operator_db(ctx: &AppContext, headers: &HeaderMap) -> Result<(), ApiError> {
    let user_id =
        dev_token_user_id(headers)?.ok_or_else(|| ApiError::forbidden("需要平台运营员登录态"))?;
    let has_operator_workspace =
        crate::repositories::auth::list_current_workspaces(&ctx.db, Some(user_id))
            .await
            .map_err(db_error)?
            .into_iter()
            .any(|workspace| {
                workspace.workspace_type == WorkspaceType::Platform
                    && workspace.role == WorkspaceRole::PlatformOperator
            });
    if has_operator_workspace {
        Ok(())
    } else {
        Err(ApiError::forbidden("需要平台运营员权限"))
    }
}

#[cfg(feature = "db")]
pub async fn child_classroom_scope(
    ctx: &AppContext,
    headers: &HeaderMap,
    workspace_id: Uuid,
    workspace: &Workspace,
) -> Result<Option<Vec<String>>, ApiError> {
    if workspace.role != WorkspaceRole::SchoolTeacher {
        return Ok(None);
    }
    let user_id = dev_token_user_id(headers)?.unwrap_or(crate::repositories::auth::DEMO_USER_ID);
    let classrooms = crate::repositories::organization::authorized_classrooms_for_user(
        &ctx.db,
        workspace_id,
        user_id,
    )
    .await
    .map_err(db_error)?;
    if classrooms.iter().any(|name| name == "全部") {
        Ok(None)
    } else {
        Ok(Some(classrooms))
    }
}

#[cfg(feature = "db")]
pub fn ensure_child_classroom_allowed(
    allowed_classrooms: Option<&[String]>,
    target_classroom: Option<&str>,
) -> Result<(), ApiError> {
    let Some(allowed_classrooms) = allowed_classrooms else {
        return Ok(());
    };
    let Some(target_classroom) = target_classroom
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Err(ApiError::forbidden("需要选择已授权班级"));
    };
    if allowed_classrooms
        .iter()
        .any(|name| name == target_classroom)
    {
        Ok(())
    } else {
        Err(ApiError::forbidden("无权访问该班级儿童档案"))
    }
}

pub fn required(value: String, field: &'static str) -> Result<String, ApiError> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(ApiError::validation(field, "字段不能为空"));
    }
    Ok(value)
}

pub fn clean_string_list(items: Vec<String>) -> Vec<String> {
    let mut values = Vec::new();
    for item in items {
        let item = item.trim().to_string();
        if !item.is_empty() && !values.contains(&item) {
            values.push(item);
        }
    }
    values
}

#[cfg(not(feature = "db"))]
pub fn paginate_vec<T>(
    items: Vec<T>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> (Vec<T>, PaginationMeta) {
    let total = items.len();
    let offset = offset.unwrap_or(0).min(total);
    let limit = limit.unwrap_or(50).clamp(1, 100);
    let has_more = offset.saturating_add(limit) < total;
    let data = items.into_iter().skip(offset).take(limit).collect();
    (
        data,
        PaginationMeta {
            total,
            limit,
            offset,
            has_more,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn auth_headers(token: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
        );
        headers
    }

    #[test]
    fn bearer_token_requires_authorization_header() {
        let headers = HeaderMap::new();
        assert!(bearer_token(&headers).is_err());
    }

    #[test]
    fn dev_token_user_id_accepts_demo_token() {
        let headers = auth_headers(crate::state::DEV_TOKEN);
        assert_eq!(dev_token_user_id(&headers).unwrap(), None);
    }

    #[test]
    fn dev_token_user_id_accepts_user_scoped_token() {
        let user_id = Uuid::new_v4();
        let headers = auth_headers(&format!("{}:{user_id}", crate::state::DEV_TOKEN));
        assert_eq!(dev_token_user_id(&headers).unwrap(), Some(user_id));
    }

    #[test]
    fn dev_token_user_id_rejects_invalid_tokens() {
        assert!(dev_token_user_id(&auth_headers("wrong-token")).is_err());
        assert!(dev_token_user_id(&auth_headers("dev-token:not-a-uuid")).is_err());
    }

    #[test]
    fn clean_string_list_trims_deduplicates_and_drops_empty_items() {
        let cleaned = clean_string_list(vec![
            " 小一班 ".to_string(),
            "".to_string(),
            "小一班".to_string(),
            "中一班".to_string(),
        ]);
        assert_eq!(cleaned, vec!["小一班".to_string(), "中一班".to_string()]);
    }

    #[cfg(feature = "db")]
    #[test]
    fn ensure_child_classroom_allowed_accepts_authorized_classroom() {
        let classrooms = vec!["小一班".to_string(), "中一班".to_string()];
        assert!(ensure_child_classroom_allowed(Some(&classrooms), Some("小一班")).is_ok());
    }

    #[cfg(feature = "db")]
    #[test]
    fn ensure_child_classroom_allowed_requires_authorized_classroom() {
        let classrooms = vec!["小一班".to_string()];
        assert!(ensure_child_classroom_allowed(Some(&classrooms), Some("大一班")).is_err());
    }

    #[cfg(feature = "db")]
    #[test]
    fn ensure_child_classroom_allowed_requires_classroom_when_scoped() {
        let classrooms = vec!["小一班".to_string()];
        assert!(ensure_child_classroom_allowed(Some(&classrooms), None).is_err());
    }
}
