use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use uuid::Uuid;

/// 绘本角色之外的常见"代称动物"黑名单：出现在正文/插图描述里但没有对应已确认角色时，
/// 说明生成结果引入了未确认形象。质量检查与 provider 输出校验共用这一份名单。
pub const UNEXPECTED_ANIMAL_NAMES: &[&str] = &[
    "小象",
    "小兔",
    "兔子",
    "小猴",
    "小熊",
    "小猫",
    "小狗",
    "小狐狸",
    "小鹿",
];

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
    pub review_status: String,
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<String>,
    pub image_url: Option<String>,
    pub selected_image_variant_id: Option<Uuid>,
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
    pub selected_image_variant_id: Option<Uuid>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StorybookImageVariant {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub storybook_id: Uuid,
    pub target_type: String,
    pub target_id: Uuid,
    pub generation_job_id: Option<Uuid>,
    pub image_url: Option<String>,
    pub prompt: Option<String>,
    pub provider: Option<String>,
    pub status: String,
    pub failure_reason: Option<String>,
    pub is_selected: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ImageVariantListQuery {
    pub target_type: Option<String>,
    pub target_id: Option<Uuid>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StorybookQualityStatus {
    Passed,
    NeedsReview,
    Blocked,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StorybookQualityCheck {
    pub key: String,
    pub label: String,
    pub status: StorybookQualityStatus,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StorybookPageQuality {
    pub page_id: Uuid,
    pub page_number: u32,
    pub status: StorybookQualityStatus,
    pub issues: Vec<String>,
    pub suggestions: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StorybookQualityReport {
    pub status: StorybookQualityStatus,
    pub summary: String,
    pub checks: Vec<StorybookQualityCheck>,
    pub pages: Vec<StorybookPageQuality>,
}

impl Default for StorybookQualityReport {
    fn default() -> Self {
        Self {
            status: StorybookQualityStatus::NeedsReview,
            summary: "等待生成质量检查".to_string(),
            checks: Vec::new(),
            pages: Vec::new(),
        }
    }
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customization_run_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customization_run_item_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customization_plan: Option<JsonValue>,
    pub creator_name: String,
    pub updated_at: String,
    pub age_group: String,
    pub use_scene: String,
    pub teaching_goal: String,
    pub cover_tone: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub story_style_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visual_style_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visual_style_version: Option<i32>,
    pub page_aspect_ratio: String,
    pub teacher_review_status: String,
    pub teacher_reviewed_by: Option<Uuid>,
    pub teacher_reviewed_at: Option<String>,
    pub pages: Vec<StorybookPage>,
    pub roles: Vec<StorybookRole>,
    pub quality: StorybookQualityReport,
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
    #[allow(dead_code)]
    #[serde(skip)]
    pub created_by: Option<Uuid>,
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
    #[serde(skip)]
    pub created_by: Option<Uuid>,
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
pub struct CreationUnderstanding {
    pub summary: String,
    pub target_user: String,
    pub goal: String,
    pub tone: String,
    pub scene: String,
    pub age_group: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_source: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quality_flags: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreationMaterial {
    pub id: String,
    pub label: String,
    #[serde(rename = "type")]
    pub material_type: String,
    pub source: String,
    pub confidence: Option<f64>,
    pub locked: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StorybookAssetSummary {
    pub id: Uuid,
    #[serde(skip_serializing)]
    pub storage_key: String,
    pub status: String,
    pub processing_message: Option<String>,
    pub content_type: String,
    pub byte_size: i64,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub visibility_scope: String,
    pub retention_policy: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StorybookVisualReferenceSummary {
    pub id: Uuid,
    pub status: String,
    pub generation_job_id: Option<Uuid>,
    pub preview_url: Option<String>,
    pub failure_reason: Option<String>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub confirmed_by: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style_version: Option<i32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StorybookAssetReference {
    pub id: Uuid,
    pub asset_id: Uuid,
    pub asset: StorybookAssetSummary,
    pub kind: String,
    pub display_name: String,
    pub usage: Option<String>,
    pub status: String,
    pub material_id: Option<String>,
    pub preview_url: Option<String>,
    pub visual_reference: Option<StorybookVisualReferenceSummary>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub revoked_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StorybookAssetUploadPolicy {
    pub max_files: u32,
    pub remaining_slots: u32,
    pub max_file_size_bytes: u64,
    pub accepted_content_types: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StorybookAssetReferenceResponse {
    pub asset_reference: StorybookAssetReference,
    pub remaining_slots: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StorybookVisualReferenceResponse {
    pub visual_reference: StorybookVisualReferenceSummary,
    pub next_action: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StorybookAssetReferenceDeleteResponse {
    pub id: Uuid,
    pub status: String,
    pub remaining_slots: u32,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStorybookAssetReferenceRequest {
    pub kind: Option<String>,
    pub display_name: Option<String>,
    pub usage: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GenerateStorybookVisualReferenceRequest {
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoryDirection {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub fit_reason: String,
    pub personal_hook: String,
    pub material_ids: Vec<String>,
    pub tone: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_source: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quality_flags: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreationOutlinePage {
    pub page_number: u32,
    pub summary: String,
    pub material_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreationOutline {
    pub summary: String,
    pub pages: Vec<CreationOutlinePage>,
    pub review_points: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_source: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quality_flags: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VisualPreferences {
    pub style: String,
    pub page_aspect_ratio: String,
    pub visual_complexity: String,
    pub character_consistency: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreationGenerationSummary {
    pub text_generation_status: String,
    pub image_generation_status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality_notice: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recoverable_actions: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StorybookCreationSession {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub created_by: Uuid,
    pub entry_type: String,
    pub source_storybook_id: Option<Uuid>,
    pub status: String,
    pub quick_idea: String,
    pub use_scene: String,
    pub age_group: String,
    pub page_count: u32,
    pub understanding: CreationUnderstanding,
    pub materials: Vec<CreationMaterial>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub asset_references: Vec<StorybookAssetReference>,
    pub directions: Vec<StoryDirection>,
    pub selected_direction_id: Option<String>,
    pub outline: Option<CreationOutline>,
    pub visual_preferences: VisualPreferences,
    pub story_style_id: String,
    pub visual_style_id: String,
    pub visual_style_version: i32,
    pub storybook_id: Option<Uuid>,
    pub last_job_id: Option<Uuid>,
    pub idempotency_key: Option<String>,
    pub generation_summary: CreationGenerationSummary,
    pub requires_understanding_refresh: bool,
    pub requires_direction_refresh: bool,
    pub requires_outline_refresh: bool,
    pub next_action: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StorybookCreationSessionListItem {
    pub id: Uuid,
    pub status: String,
    pub quick_idea: String,
    pub understanding_summary: String,
    pub selected_direction_title: Option<String>,
    pub storybook_id: Option<Uuid>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreationSessionUpdateResponse {
    pub id: Uuid,
    pub status: String,
    pub requires_understanding_refresh: bool,
    pub requires_direction_refresh: bool,
    pub requires_outline_refresh: bool,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreationMaterialsResponse {
    pub id: Uuid,
    pub status: String,
    pub materials: Vec<CreationMaterial>,
    pub requires_direction_refresh: bool,
    pub requires_outline_refresh: bool,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreationDirectionsResponse {
    pub status: String,
    pub directions: Vec<StoryDirection>,
    pub next_action: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelectDirectionResponse {
    pub status: String,
    pub selected_direction_id: String,
    pub selected_direction: StoryDirection,
    pub next_action: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreationOutlineResponse {
    pub status: String,
    pub outline: CreationOutline,
    pub next_action: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateOutlinePageResponse {
    pub page: CreationOutlinePage,
    pub requires_storybook_regeneration: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateOutlineResponse {
    pub status: String,
    pub outline: CreationOutline,
    pub requires_storybook_regeneration: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VisualPreferencesResponse {
    pub id: Uuid,
    pub status: String,
    pub visual_preferences: VisualPreferences,
    pub requires_storybook_regeneration: bool,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CreativeSettingsEffects {
    pub references_invalidated: bool,
    pub invalidated_asset_reference_ids: Vec<Uuid>,
    pub requires_direction_refresh: bool,
    pub requires_outline_refresh: bool,
    pub requires_storybook_regeneration: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct CreativeSettingsResponse {
    pub session: StorybookCreationSession,
    pub effects: CreativeSettingsEffects,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreationGenerationStep {
    pub key: String,
    pub label: String,
    pub status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreationStorybookGenerationResponse {
    pub status: String,
    pub storybook_id: Option<Uuid>,
    pub job_id: Option<Uuid>,
    pub generation_summary: CreationGenerationSummary,
    pub steps: Vec<CreationGenerationStep>,
    pub next_action: String,
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
    pub meta: Option<ResponseMeta>,
}

impl<T> Envelope<T> {
    pub fn new(data: T) -> Self {
        Self { data, meta: None }
    }

    pub fn with_meta(data: T, meta: PaginationMeta) -> Self {
        Self {
            data,
            meta: Some(ResponseMeta::from(meta)),
        }
    }

    pub fn with_warnings(data: T, warnings: Vec<ResponseWarning>) -> Self {
        if warnings.is_empty() {
            Self::new(data)
        } else {
            Self {
                data,
                meta: Some(ResponseMeta {
                    warnings,
                    ..ResponseMeta::default()
                }),
            }
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

#[derive(Clone, Debug, Default, Serialize)]
pub struct ResponseMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_more: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<ResponseWarning>,
}

impl From<PaginationMeta> for ResponseMeta {
    fn from(value: PaginationMeta) -> Self {
        Self {
            total: Some(value.total),
            limit: Some(value.limit),
            offset: Some(value.offset),
            has_more: Some(value.has_more),
            warnings: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ResponseWarning {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub asset_reference_ids: Vec<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_action: Option<String>,
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
pub struct StorybookCreationSessionListQuery {
    pub status: Option<String>,
    pub created_by: Option<Uuid>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct CreateStorybookCreationSessionRequest {
    pub quick_idea: String,
    #[serde(default)]
    pub entry_type: Option<String>,
    #[serde(default)]
    pub source_storybook_id: Option<Uuid>,
    pub use_scene: Option<String>,
    pub age_group: Option<String>,
    pub page_count: Option<u32>,
    pub style: Option<String>,
    #[serde(default)]
    pub story_style_id: Option<String>,
    #[serde(default)]
    pub visual_style_id: Option<String>,
    pub page_aspect_ratio: Option<String>,
    pub visual_complexity: Option<String>,
    pub character_consistency: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStorybookCreationSessionRequest {
    pub quick_idea: Option<String>,
    pub use_scene: Option<String>,
    pub age_group: Option<String>,
    pub page_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct RefreshUnderstandingRequest {
    pub preserve_user_materials: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct MaterialOperation {
    pub op: String,
    pub id: Option<String>,
    pub label: Option<String>,
    #[serde(rename = "type")]
    pub material_type: Option<String>,
    pub locked: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct PatchCreationMaterialsRequest {
    pub operations: Vec<MaterialOperation>,
}

#[derive(Debug, Deserialize)]
pub struct GenerateDirectionsRequest {
    pub direction_count: Option<u32>,
    pub refresh_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SelectDirectionRequest {
    pub direction_id: String,
}

#[derive(Debug, Deserialize)]
pub struct GenerateOutlineRequest {
    pub page_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateOutlinePageRequest {
    pub instruction: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCreationOutlineRequest {
    pub summary: String,
    pub pages: Vec<CreationOutlinePage>,
    #[serde(default)]
    pub review_points: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateVisualPreferencesRequest {
    pub style: Option<String>,
    pub page_aspect_ratio: Option<String>,
    pub visual_complexity: Option<String>,
    pub character_consistency: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCreativeSettingsRequest {
    pub story_style_id: Option<String>,
    pub visual_style_id: Option<String>,
    pub page_count: Option<u32>,
    pub page_aspect_ratio: Option<String>,
    pub visual_complexity: Option<String>,
    pub character_consistency: Option<String>,
    #[serde(default)]
    pub confirm_reference_regeneration: bool,
}

#[derive(Debug, Deserialize)]
pub struct GenerateCreationStorybookRequest {
    pub generation_mode: String,
    pub include_images: Option<bool>,
    pub idempotency_key: String,
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
    /// 可选：用户选择的画风描述，作为绘本级画风持久化（供角色参考图/插图拼接）。
    pub cover_tone: Option<String>,
    #[serde(default)]
    pub story_style_id: Option<String>,
    #[serde(default)]
    pub visual_style_id: Option<String>,
    #[serde(default)]
    pub visual_style_version: Option<i32>,
    pub page_aspect_ratio: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct DuplicateStorybookRequest {
    pub title: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStorybookRequest {
    pub title: Option<String>,
    pub status: Option<StorybookStatus>,
    pub visibility: Option<Visibility>,
    pub teacher_review_status: Option<String>,
    pub age_group: Option<String>,
    pub use_scene: Option<String>,
    pub teaching_goal: Option<String>,
    pub cover_tone: Option<String>,
    pub page_aspect_ratio: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePageRequest {
    pub title: Option<String>,
    pub body: Option<String>,
    pub illustration_prompt: Option<String>,
    pub status: Option<String>,
    pub review_status: Option<String>,
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

#[derive(Debug, Default, Deserialize)]
pub struct CreateBulkImageTasksRequest {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BulkImageTaskResponse {
    pub jobs: Vec<GenerationJob>,
    pub concurrency_limit: u32,
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
    pub primary_material: Option<String>,
    #[serde(default)]
    pub customization_plan: Option<JsonValue>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BuildCustomizationPlanRequest {
    pub mode: String,
    #[serde(default)]
    pub target_child_id: Option<Uuid>,
    #[serde(default)]
    pub target_child_ids: Vec<Uuid>,
    #[serde(default)]
    pub primary_material: Option<String>,
    #[serde(default)]
    pub optional_keep_page_ids: Vec<Uuid>,
    #[serde(default)]
    pub confirmed_photo_reference_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeriveCustomBatchRequest {
    pub child_ids: Vec<Uuid>,
    pub intensity: String,
    #[serde(default)]
    pub customization_plan: Option<JsonValue>,
    #[serde(default)]
    pub material_choices: HashMap<Uuid, String>,
    #[serde(default)]
    pub creation_session_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeriveCustomBatchResponse {
    pub source_storybook_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<Uuid>,
    pub requested_count: usize,
    pub created_count: usize,
    pub storybooks: Vec<Storybook>,
    #[serde(default)]
    pub items: Vec<DeriveCustomBatchItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeriveCustomBatchItem {
    pub child_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_item_id: Option<Uuid>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storybook: Option<Storybook>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorybookCustomizationRun {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub source_storybook_id: Uuid,
    pub created_by: Uuid,
    pub entry_type: String,
    pub mode: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customization_plan: Option<JsonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_snapshot: Option<JsonValue>,
    pub requested_count: usize,
    pub succeeded_count: usize,
    pub failed_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub items: Vec<StorybookCustomizationRunItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorybookCustomizationRunItem {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub run_id: Uuid,
    pub source_storybook_id: Uuid,
    pub target_child_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_child_nickname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_storybook_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_storybook_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_material: Option<String>,
    pub status: String,
    pub generation_input_snapshot: JsonValue,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::{ExportJob, GenerationJob, StorybookAssetSummary};
    use chrono::Utc;
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn internal_storage_owner_fields_are_not_serialized() {
        let user_id = Uuid::new_v4();
        let generation_job = GenerationJob {
            id: Uuid::new_v4(),
            workspace_id: Uuid::new_v4(),
            storybook_id: Some(Uuid::new_v4()),
            created_by: Some(user_id),
            job_type: "storybook_page_image".to_string(),
            status: "succeeded".to_string(),
            input_json: json!({"prompt": "safe prompt"}),
            output_json: None,
            attempt_count: 1,
            last_error: None,
            next_run_at: None,
            locked_by: None,
            locked_at: None,
            created_at: Utc::now(),
            finished_at: None,
        };
        let export_job = ExportJob {
            id: Uuid::new_v4(),
            storybook_id: Uuid::new_v4(),
            created_by: Some(user_id),
            status: "succeeded".to_string(),
            file_url: Some("/api/example/download".to_string()),
            last_error: None,
            created_at: Utc::now(),
            finished_at: None,
        };

        let generation_json =
            serde_json::to_value(generation_job).expect("generation job should serialize");
        let export_json = serde_json::to_value(export_job).expect("export job should serialize");

        assert!(generation_json.get("created_by").is_none());
        assert!(export_json.get("created_by").is_none());
    }

    #[test]
    fn storybook_asset_storage_key_is_not_serialized() {
        let asset = StorybookAssetSummary {
            id: Uuid::new_v4(),
            storage_key: "/storybook-assets/private.png".to_string(),
            status: "ready".to_string(),
            processing_message: None,
            content_type: "image/png".to_string(),
            byte_size: 128,
            width: Some(512),
            height: Some(512),
            visibility_scope: "creation_session".to_string(),
            retention_policy: "session_scoped".to_string(),
        };

        let asset_json = serde_json::to_value(asset).expect("asset should serialize");

        assert!(asset_json.get("storage_key").is_none());
        assert_eq!(asset_json["visibility_scope"], "creation_session");
        assert_eq!(asset_json["retention_policy"], "session_scoped");
    }
}
