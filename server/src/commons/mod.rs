pub mod error;
pub mod state;
pub mod support;

pub use error::{ApiError, ApiErrorBody, ApiErrorDetail, ErrorEnvelope};
pub use state::{AppState, SharedState, shared_demo_state};
pub use support::{demo_uuid, now};
