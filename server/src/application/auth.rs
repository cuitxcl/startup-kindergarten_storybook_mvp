use axum::http::HeaderMap;
use loco_rs::app::AppContext;
use uuid::Uuid;

use crate::{
    domains::common,
    error::ApiError,
    models::{LoginRequest, LoginResponse, RegisterRequest, WorkspaceInvitationDetail},
};

#[cfg(not(feature = "db"))]
use crate::models::{User, WorkspaceRole};

pub async fn login(ctx: &AppContext, payload: LoginRequest) -> Result<LoginResponse, ApiError> {
    if payload.identifier.trim().is_empty() {
        return Err(ApiError::validation("identifier", "请输入邮箱或手机号"));
    }
    if payload.password.trim().is_empty() {
        return Err(ApiError::validation("password", "请输入密码"));
    }

    #[cfg(feature = "db")]
    {
        return crate::repositories::auth::login(
            &ctx.db,
            payload.identifier.trim(),
            &payload.password,
        )
        .await
        .map_err(common::db_error);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        let state = state.read().expect("state lock poisoned");
        Ok(LoginResponse {
            token: state.token.clone(),
            user: state.current_user.clone(),
            workspaces: state.workspaces.clone(),
        })
    }
}

pub async fn register(
    ctx: &AppContext,
    payload: RegisterRequest,
) -> Result<LoginResponse, ApiError> {
    let display_name = common::required(payload.display_name, "display_name")?;
    let email = common::required(payload.email, "email")?;
    #[cfg(feature = "db")]
    let password = common::required(payload.password, "password")?;
    #[cfg(not(feature = "db"))]
    let _password = common::required(payload.password, "password")?;

    #[cfg(feature = "db")]
    {
        return crate::repositories::auth::register(
            &ctx.db,
            RegisterRequest {
                display_name,
                email,
                password,
            },
        )
        .await
        .map_err(common::db_error);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        let state = state.read().expect("state lock poisoned");
        Ok(LoginResponse {
            token: state.token.clone(),
            user: User {
                id: Uuid::new_v4(),
                display_name,
                email,
            },
            workspaces: state.workspaces.clone(),
        })
    }
}

pub async fn current_session(
    ctx: &AppContext,
    headers: &HeaderMap,
) -> Result<LoginResponse, ApiError> {
    #[cfg(feature = "db")]
    {
        let user_id = common::dev_token_user_id(headers)?;
        return crate::repositories::auth::current_session(&ctx.db, user_id)
            .await
            .map_err(common::db_error);
    }

    #[cfg(not(feature = "db"))]
    {
        let state = shared_state(ctx)?;
        common::require_login(&state, headers)?;
        let state = state.read().expect("state lock poisoned");
        Ok(LoginResponse {
            token: state.token.clone(),
            user: state.current_user.clone(),
            workspaces: state.workspaces.clone(),
        })
    }
}

pub async fn get_invitation(
    ctx: &AppContext,
    token: Uuid,
) -> Result<WorkspaceInvitationDetail, ApiError> {
    #[cfg(feature = "db")]
    {
        return crate::repositories::organization::get_invitation(&ctx.db, token)
            .await
            .map_err(common::db_error);
    }

    #[cfg(not(feature = "db"))]
    {
        let _ = ctx;
        Ok(mock_invitation(token, "invited"))
    }
}

pub async fn accept_invitation(
    ctx: &AppContext,
    token: Uuid,
) -> Result<WorkspaceInvitationDetail, ApiError> {
    #[cfg(feature = "db")]
    {
        return crate::repositories::organization::accept_invitation(&ctx.db, token)
            .await
            .map_err(common::db_error);
    }

    #[cfg(not(feature = "db"))]
    {
        let _ = ctx;
        Ok(mock_invitation(token, "active"))
    }
}

#[cfg(not(feature = "db"))]
fn mock_invitation(token: Uuid, status: &str) -> WorkspaceInvitationDetail {
    WorkspaceInvitationDetail {
        token: token.to_string(),
        workspace_id: Uuid::from_u128(0x20000000000000000000000000000001),
        workspace_name: "星星幼儿园".to_string(),
        invited_by: "园所管理员".to_string(),
        invited_contact: "teacher@example.com".to_string(),
        role: WorkspaceRole::SchoolTeacher,
        classrooms: vec!["小一班".to_string()],
        status: status.to_string(),
    }
}

#[cfg(not(feature = "db"))]
fn shared_state(ctx: &AppContext) -> Result<crate::state::SharedState, ApiError> {
    ctx.shared_store
        .get::<crate::state::SharedState>()
        .ok_or_else(|| ApiError::state_conflict("应用状态未初始化"))
}
