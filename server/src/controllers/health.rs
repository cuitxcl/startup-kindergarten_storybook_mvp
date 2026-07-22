use axum::{Json, routing::get};
use loco_rs::controller::Routes;
use serde::Serialize;

use crate::models::Envelope;

pub fn routes() -> Routes {
    Routes::new().add("/api/health", get(health))
}

async fn health() -> Json<Envelope<HealthResponse>> {
    Json(Envelope::new(HealthResponse {
        status: "ok",
        service: "kindleaf-server",
    }))
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_include_health_check() {
        let registered_routes = routes();
        let uris = registered_routes
            .handlers
            .iter()
            .map(|handler| handler.uri.as_str())
            .collect::<Vec<_>>();

        assert_eq!(uris, vec!["/api/health"]);
    }
}
