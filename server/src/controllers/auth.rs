use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use loco_rs::{app::AppContext, controller::Routes};
use uuid::Uuid;

use crate::{
    application,
    error::ApiError,
    models::{Envelope, LoginRequest, LoginResponse, RegisterRequest, WorkspaceInvitationDetail},
};

pub fn routes() -> Routes {
    Routes::new()
        .add("/api/auth/login", post(login))
        .add("/api/auth/register", post(register))
        .add("/api/auth/me", get(me))
        .add("/api/invitations/{token}", get(get_invitation))
        .add("/api/invitations/{token}/accept", post(accept_invitation))
}

async fn login(
    State(ctx): State<AppContext>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<Envelope<LoginResponse>>, ApiError> {
    let response = application::auth::login(&ctx, payload).await?;
    Ok(Json(Envelope::new(response)))
}

async fn register(
    State(ctx): State<AppContext>,
    Json(payload): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<Envelope<LoginResponse>>), ApiError> {
    let response = application::auth::register(&ctx, payload).await?;
    Ok((StatusCode::CREATED, Json(Envelope::new(response))))
}

async fn me(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
) -> Result<Json<Envelope<LoginResponse>>, ApiError> {
    let response = application::auth::current_session(&ctx, &headers).await?;
    Ok(Json(Envelope::new(response)))
}

async fn get_invitation(
    State(ctx): State<AppContext>,
    Path(token): Path<Uuid>,
) -> Result<Json<Envelope<WorkspaceInvitationDetail>>, ApiError> {
    let invitation = application::auth::get_invitation(&ctx, token).await?;
    Ok(Json(Envelope::new(invitation)))
}

async fn accept_invitation(
    State(ctx): State<AppContext>,
    Path(token): Path<Uuid>,
) -> Result<Json<Envelope<WorkspaceInvitationDetail>>, ApiError> {
    let invitation = application::auth::accept_invitation(&ctx, token).await?;
    Ok(Json(Envelope::new(invitation)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_include_auth_and_invitation_api() {
        let registered_routes = routes();
        let uris = registered_routes
            .handlers
            .iter()
            .map(|handler| handler.uri.as_str())
            .collect::<Vec<_>>();

        assert!(uris.contains(&"/api/auth/login"));
        assert!(uris.contains(&"/api/auth/register"));
        assert!(uris.contains(&"/api/auth/me"));
        assert!(uris.contains(&"/api/invitations/{token}"));
        assert!(uris.contains(&"/api/invitations/{token}/accept"));
    }
}
