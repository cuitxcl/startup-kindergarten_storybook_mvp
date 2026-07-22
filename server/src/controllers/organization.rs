use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use loco_rs::{app::AppContext, controller::Routes};
use uuid::Uuid;

use crate::{
    application,
    error::ApiError,
    models::{
        AuditLogEntry, Classroom, CreateClassroomRequest, CreateMemberRequest, Envelope, ListQuery,
        WorkspaceMember,
    },
};

pub fn routes() -> Routes {
    Routes::new()
        .add(
            "/api/workspaces/{workspace_id}/members",
            get(list_members).post(create_member),
        )
        .add(
            "/api/workspaces/{workspace_id}/members/{member_id}/revoke-invitation",
            post(revoke_member_invitation),
        )
        .add(
            "/api/workspaces/{workspace_id}/classes",
            get(list_classes).post(create_classroom),
        )
        .add(
            "/api/workspaces/{workspace_id}/classes/{classroom_id}/archive",
            post(archive_classroom),
        )
        .add(
            "/api/workspaces/{workspace_id}/audit-logs",
            get(list_audit_logs),
        )
}

async fn list_members(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Envelope<Vec<WorkspaceMember>>>, ApiError> {
    let (members, meta) =
        application::organization::list_members(&ctx, &headers, workspace_id, query).await?;
    Ok(Json(Envelope::with_meta(members, meta)))
}

async fn create_member(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Json(payload): Json<CreateMemberRequest>,
) -> Result<(StatusCode, Json<Envelope<WorkspaceMember>>), ApiError> {
    let member =
        application::organization::create_member(&ctx, &headers, workspace_id, payload).await?;
    Ok((StatusCode::CREATED, Json(Envelope::new(member))))
}

async fn revoke_member_invitation(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, member_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Envelope<WorkspaceMember>>, ApiError> {
    let member = application::organization::revoke_member_invitation(
        &ctx,
        &headers,
        workspace_id,
        member_id,
    )
    .await?;
    Ok(Json(Envelope::new(member)))
}

async fn list_classes(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Envelope<Vec<Classroom>>>, ApiError> {
    let (classrooms, meta) =
        application::organization::list_classrooms(&ctx, &headers, workspace_id, query).await?;
    Ok(Json(Envelope::with_meta(classrooms, meta)))
}

async fn create_classroom(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Json(payload): Json<CreateClassroomRequest>,
) -> Result<(StatusCode, Json<Envelope<Classroom>>), ApiError> {
    let classroom =
        application::organization::create_classroom(&ctx, &headers, workspace_id, payload).await?;
    Ok((StatusCode::CREATED, Json(Envelope::new(classroom))))
}

async fn archive_classroom(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((workspace_id, classroom_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Envelope<Classroom>>, ApiError> {
    let classroom =
        application::organization::archive_classroom(&ctx, &headers, workspace_id, classroom_id)
            .await?;
    Ok(Json(Envelope::new(classroom)))
}

async fn list_audit_logs(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Envelope<Vec<AuditLogEntry>>>, ApiError> {
    let (logs, meta) =
        application::organization::list_audit_logs(&ctx, &headers, workspace_id, query).await?;
    Ok(Json(Envelope::with_meta(logs, meta)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_include_organization_api() {
        let registered_routes = routes();
        let uris = registered_routes
            .handlers
            .iter()
            .map(|handler| handler.uri.as_str())
            .collect::<Vec<_>>();

        assert!(uris.contains(&"/api/workspaces/{workspace_id}/members"));
        assert!(
            uris.contains(&"/api/workspaces/{workspace_id}/members/{member_id}/revoke-invitation")
        );
        assert!(uris.contains(&"/api/workspaces/{workspace_id}/classes"));
        assert!(uris.contains(&"/api/workspaces/{workspace_id}/classes/{classroom_id}/archive"));
        assert!(uris.contains(&"/api/workspaces/{workspace_id}/audit-logs"));
    }
}
