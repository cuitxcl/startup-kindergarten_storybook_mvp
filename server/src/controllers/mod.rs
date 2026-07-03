//! Backward-compatible controller facade.
//!
//! API handlers live in `crate::api`; response DTOs live in `crate::views`.

pub use crate::api::{
    auth, children, content, dashboard, delivery, images, organization, router as api, storybooks,
    visuals,
};
pub use crate::commons::{ApiError, SharedState, now};
