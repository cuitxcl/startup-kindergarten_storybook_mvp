use axum::{Router, middleware, routing::post};

use super::{
    auth, children, content, dashboard, delivery, images, organization, storybooks, visuals,
};
use crate::commons::SharedState;

pub fn router(state: SharedState) -> Router {
    let public_routes = Router::new()
        .nest("/api", auth::router())
        .route("/api/parent-intakes", post(children::create_parent_intake));

    let protected_routes = Router::new()
        .nest("/api", organization::router())
        .nest("/api", dashboard::router())
        .nest("/api", children::protected_router())
        .nest("/api", content::router())
        .nest("/api", delivery::router())
        .nest("/api", images::router())
        .nest("/api", storybooks::router())
        .nest("/api", visuals::router())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_session,
        ));

    public_routes.merge(protected_routes).with_state(state)
}
