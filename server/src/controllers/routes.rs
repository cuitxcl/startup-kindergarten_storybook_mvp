use loco_rs::controller::Routes;

use super::{
    auth, children, delivery, generation, health, marketplace, operator, organization,
    parent_intakes, storybooks, submissions, workspaces,
};

pub fn routes() -> Vec<Routes> {
    vec![
        health::routes(),
        auth::routes(),
        workspaces::routes(),
        organization::routes(),
        children::routes(),
        parent_intakes::routes(),
        marketplace::routes(),
        submissions::routes(),
        storybooks::routes(),
        generation::routes(),
        delivery::routes(),
        operator::routes(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_include_health_and_business_api() {
        let registered_routes = routes();
        let uris = registered_routes
            .iter()
            .flat_map(|routes| routes.handlers.iter())
            .map(|handler| handler.uri.as_str())
            .collect::<Vec<_>>();

        assert!(uris.contains(&"/api/health"));
        assert!(uris.contains(&"/api/auth/me"));
        assert!(uris.contains(&"/api/workspaces"));
        assert!(uris.contains(&"/api/workspaces/{workspace_id}/children"));
        assert!(uris.contains(&"/api/marketplace/templates"));
        assert!(uris.contains(&"/api/workspaces/{workspace_id}/submissions"));
        assert!(uris.contains(&"/api/workspaces/{workspace_id}/storybooks"));
        assert!(uris.contains(&"/api/workspaces/{workspace_id}/generation-jobs"));
        assert!(uris.contains(&"/api/share-links/{token}"));
        assert!(uris.contains(&"/api/operator/readiness"));
    }
}
