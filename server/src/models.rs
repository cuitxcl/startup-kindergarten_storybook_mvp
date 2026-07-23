use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceRole {
    PersonalOwner,
    SchoolTeacher,
    SchoolAdmin,
    PlatformOperator,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceType {
    Personal,
    School,
    Platform,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StorybookType {
    Plain,
    Custom,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StorybookStatus {
    Draft,
    PlanPending,
    RolesPending,
    Editing,
    ImagePending,
    Exportable,
    Submitted,
    Listed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Private,
    Workspace,
    MarketSubmission,
    MarketListed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub display_name: String,
    pub email: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Workspace {
    pub id: Uuid,
    pub name: String,
    #[serde(rename = "type")]
    pub workspace_type: WorkspaceType,
    pub role: WorkspaceRole,
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceMember {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub email: String,
    pub role: WorkspaceRole,
    pub status: String,
    pub classes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invitation_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invitation_url: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceInvitationDetail {
    pub token: String,
    pub workspace_id: Uuid,
    pub workspace_name: String,
    pub invited_by: String,
    pub invited_contact: String,
    pub role: WorkspaceRole,
    pub classrooms: Vec<String>,
    pub status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Classroom {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub age_group: String,
    pub teachers: u32,
    pub children: u32,
    pub status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChildProfile {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub nickname: String,
    pub age_group: String,
    pub classroom: Option<String>,
    pub interests: Vec<String>,
    pub traits: Vec<String>,
    pub focus: String,
    pub completeness: u8,
    pub status: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParentIntake {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub child_nickname: String,
    pub age_group: String,
    pub classroom: Option<String>,
    pub interests: Vec<String>,
    pub status: String,
    pub confirmed_child_id: Option<Uuid>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParentIntakeLink {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub token: String,
    pub label: String,
    pub classroom: Option<String>,
    pub status: String,
    pub url: String,
    pub expires_at: Option<String>,
    pub access_count: i32,
    pub last_accessed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicParentIntakeLink {
    pub token: String,
    pub workspace_id: Uuid,
    pub workspace_name: String,
    pub label: String,
    pub classroom: Option<String>,
    pub status: String,
    pub expires_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StorybookPage {
    pub id: Uuid,
    pub page_number: u32,
    pub title: String,
    pub body: String,
    pub illustration_prompt: String,
    pub status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StorybookRole {
    pub id: Uuid,
    pub name: String,
    pub role_type: String,
    pub appearance: String,
    pub story_function: String,
    pub needs_consistency: bool,
    pub reference_image_url: Option<String>,
    pub reference_image_prompt: Option<String>,
    pub reference_status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Storybook {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub title: String,
    #[serde(rename = "type")]
    pub storybook_type: StorybookType,
    pub status: StorybookStatus,
    pub visibility: Visibility,
    pub source: String,
    pub source_title: Option<String>,
    pub target_child_id: Option<Uuid>,
    pub creator_name: String,
    pub updated_at: String,
    pub age_group: String,
    pub use_scene: String,
    pub teaching_goal: String,
    pub cover_tone: String,
    pub pages: Vec<StorybookPage>,
    pub roles: Vec<StorybookRole>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarketplaceTemplate {
    pub id: Uuid,
    pub title: String,
    pub summary: String,
    pub source_type: String,
    pub source_label: String,
    pub source_storybook_id: Option<Uuid>,
    pub age_group: String,
    pub use_scene: String,
    pub page_count: u32,
    pub supports_customization: bool,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarketplaceSubmission {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub title: String,
    pub source_storybook_title: String,
    pub submitted_by: String,
    pub status: String,
    pub privacy_confirmed: bool,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DashboardResponse {
    pub workspace: Workspace,
    pub storybooks: Vec<Storybook>,
    pub children: Vec<ChildProfile>,
    pub submissions: Vec<MarketplaceSubmission>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShareLink {
    pub id: Uuid,
    pub storybook_id: Uuid,
    pub token: String,
    pub url: String,
    pub status: String,
    pub access_count: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_accessed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct CreateShareLinkRequest {
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportJob {
    pub id: Uuid,
    pub storybook_id: Uuid,
    pub status: String,
    pub file_url: Option<String>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerationJob {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub storybook_id: Option<Uuid>,
    pub job_type: String,
    pub status: String,
    pub input_json: JsonValue,
    pub output_json: Option<JsonValue>,
    pub attempt_count: i32,
    pub last_error: Option<String>,
    pub next_run_at: Option<DateTime<Utc>>,
    pub locked_by: Option<String>,
    pub locked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub id: Uuid,
    pub workspace_id: Option<Uuid>,
    pub actor_user_id: Option<Uuid>,
    pub actor_name: Option<String>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<Uuid>,
    pub metadata_json: JsonValue,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct Envelope<T> {
    pub data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<PaginationMeta>,
}

impl<T> Envelope<T> {
    pub fn new(data: T) -> Self {
        Self { data, meta: None }
    }

    pub fn with_meta(data: T, meta: PaginationMeta) -> Self {
        Self {
            data,
            meta: Some(meta),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct PaginationMeta {
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
    pub has_more: bool,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub identifier: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub display_name: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: User,
    pub workspaces: Vec<Workspace>,
}

#[derive(Debug, Deserialize)]
pub struct CreateChildRequest {
    pub nickname: String,
    pub age_group: String,
    #[serde(default)]
    pub classroom: Option<String>,
    #[serde(default)]
    pub interests: Vec<String>,
    #[serde(default)]
    pub traits: Vec<String>,
    pub focus: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateChildRequest {
    pub nickname: Option<String>,
    pub age_group: Option<String>,
    pub classroom: Option<String>,
    pub interests: Option<Vec<String>>,
    pub traits: Option<Vec<String>>,
    pub focus: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct StorybookListQuery {
    #[serde(rename = "type")]
    pub storybook_type: Option<String>,
    pub status: Option<String>,
    pub target_child_id: Option<Uuid>,
    pub q: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct ParentIntakeLinkListQuery {
    pub status: Option<String>,
    pub classroom: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct ParentIntakeListQuery {
    pub classroom: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct ParentIntakeLinkBulkActionQuery {
    pub classroom: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SubmissionListQuery {
    pub status: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct GenerationJobListQuery {
    pub storybook_id: Option<Uuid>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct GenerationCostListQuery {
    pub workspace_id: Option<Uuid>,
    pub provider: Option<String>,
    pub job_type: Option<String>,
    pub status: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerationCostLog {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub workspace_name: Option<String>,
    pub generation_job_id: Uuid,
    pub storybook_id: Option<Uuid>,
    pub storybook_title: Option<String>,
    pub provider: String,
    pub job_type: String,
    pub status: String,
    pub estimated_input_units: i32,
    pub estimated_output_units: i32,
    pub image_count: i32,
    pub estimated_cost_micros: i64,
    pub currency: String,
    pub metadata_json: JsonValue,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerationCostSummary {
    pub total_cost_micros: i64,
    pub succeeded_cost_micros: i64,
    pub failed_jobs: i64,
    pub total_jobs: i64,
    pub total_input_units: i64,
    pub total_output_units: i64,
    pub total_images: i64,
    pub currency: String,
    pub budget_limit_micros: Option<i64>,
    pub budget_used_percent: Option<f64>,
    pub budget_warning_percent: Option<f64>,
    pub budget_warning: bool,
    pub budget_exceeded: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerationCostReport {
    pub summary: GenerationCostSummary,
    pub items: Vec<GenerationCostLog>,
}

#[derive(Debug, Deserialize)]
pub struct CreateStorybookRequest {
    pub title: String,
    pub age_group: String,
    pub use_scene: String,
    pub teaching_goal: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStorybookRequest {
    pub title: Option<String>,
    pub status: Option<StorybookStatus>,
    pub visibility: Option<Visibility>,
    pub age_group: Option<String>,
    pub use_scene: Option<String>,
    pub teaching_goal: Option<String>,
    pub cover_tone: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePageRequest {
    pub title: Option<String>,
    pub body: Option<String>,
    pub illustration_prompt: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRoleRequest {
    pub name: Option<String>,
    pub role_type: Option<String>,
    pub appearance: Option<String>,
    pub story_function: Option<String>,
    pub needs_consistency: Option<bool>,
    pub reference_image_url: Option<String>,
    pub reference_image_prompt: Option<String>,
    pub reference_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateImageTaskRequest {
    pub prompt: Option<String>,
    #[serde(default)]
    pub reference_role_ids: Vec<Uuid>,
    #[serde(default)]
    pub reference_image_urls: Vec<String>,
    #[serde(default)]
    pub edit_instruction: Option<String>,
    #[serde(default)]
    pub image_mode: Option<String>,
    #[serde(default)]
    pub strength: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateGenerationJobRequest {
    pub job_type: String,
    #[serde(default)]
    pub storybook_id: Option<Uuid>,
    #[serde(default)]
    pub input_json: JsonValue,
}

#[derive(Debug, Deserialize)]
pub struct DeriveCustomRequest {
    pub child_id: Uuid,
    pub intensity: String,
    #[serde(default)]
    pub customization_plan: Option<JsonValue>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeriveCustomBatchRequest {
    pub child_ids: Vec<Uuid>,
    pub intensity: String,
    #[serde(default)]
    pub customization_plan: Option<JsonValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeriveCustomBatchResponse {
    pub source_storybook_id: Uuid,
    pub requested_count: usize,
    pub created_count: usize,
    pub storybooks: Vec<Storybook>,
}

#[derive(Debug, Deserialize)]
pub struct MarketplaceQuery {
    pub source: Option<String>,
    pub q: Option<String>,
    pub supports_customization: Option<bool>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMarketplaceTemplateRequest {
    pub title: Option<String>,
    pub summary: Option<String>,
    pub age_group: Option<String>,
    pub use_scene: Option<String>,
    pub supports_customization: Option<bool>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateMemberRequest {
    pub email: String,
    pub name: String,
    #[serde(default)]
    pub classes: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateClassroomRequest {
    pub name: String,
    pub age_group: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateSubmissionRequest {
    pub storybook_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct ParentIntakeRequest {
    #[allow(dead_code)]
    #[serde(default)]
    pub workspace_id: Option<Uuid>,
    #[serde(default)]
    pub link_token: Option<String>,
    pub child_nickname: String,
    pub age_group: String,
    #[serde(default)]
    pub interests: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateParentIntakeLinkRequest {
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub classroom: Option<String>,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct ConfirmParentIntakeRequest {
    pub focus: Option<String>,
    #[serde(default)]
    pub traits: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ActionResponse {
    pub status: String,
    pub message: String,
}
