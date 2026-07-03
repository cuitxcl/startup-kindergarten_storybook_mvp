use std::sync::{Arc, RwLock};

use crate::models::{auth, children, content, delivery, images, organization, storybooks, visuals};

pub type SharedState = Arc<RwLock<AppState>>;

#[derive(Clone, Debug)]
pub struct AppState {
    pub auth: auth::AuthStore,
    pub children: children::ChildrenStore,
    pub content: content::ContentStore,
    pub delivery: delivery::DeliveryStore,
    pub images: images::ImageGenerationStore,
    pub organization: organization::OrganizationStore,
    pub storybooks: storybooks::StorybookStore,
    pub visuals: visuals::VisualConsistencyStore,
}

impl AppState {
    pub fn demo() -> Self {
        let organization = organization::OrganizationStore::demo();
        Self {
            auth: auth::AuthStore::demo(&organization),
            children: children::ChildrenStore::demo(&organization),
            content: content::ContentStore::demo(),
            delivery: delivery::DeliveryStore::demo(),
            images: images::ImageGenerationStore::demo(),
            organization,
            storybooks: storybooks::StorybookStore::demo(),
            visuals: visuals::VisualConsistencyStore::demo(),
        }
    }
}

pub fn shared_demo_state() -> SharedState {
    Arc::new(RwLock::new(AppState::demo()))
}
