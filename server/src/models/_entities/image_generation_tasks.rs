use sea_orm::entity::prelude::*;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "image_generation_tasks")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub idempotency_key: Option<String>,
    pub task_type: String,
    pub parent_task_id: Option<Uuid>,
    pub retry_of_task_id: Option<Uuid>,
    pub school_id: Option<Uuid>,
    pub teacher_id: Option<Uuid>,
    pub storybook_id: Option<Uuid>,
    pub storybook_page_id: Option<Uuid>,
    pub character_profile_id: Option<Uuid>,
    pub character_profile_version: Option<i32>,
    pub reference_image_id: Option<Uuid>,
    pub style_id: String,
    pub prompt_template_version: String,
    pub scene_spec_json: Option<Value>,
    pub input_snapshot_json: Value,
    pub raw_prompt_text: Option<String>,
    pub provider_name: Option<String>,
    pub model_name: Option<String>,
    pub provider_request_id: Option<String>,
    pub status: String,
    pub failure_reason: Option<String>,
    pub retry_count: i32,
    pub max_retries: i32,
    pub queued_at: DateTimeWithTimeZone,
    pub started_at: Option<DateTimeWithTimeZone>,
    pub completed_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::image_generation_tasks::Entity",
        from = "Column::ParentTaskId",
        to = "Column::Id"
    )]
    ParentTask,
    #[sea_orm(
        belongs_to = "super::image_generation_tasks::Entity",
        from = "Column::RetryOfTaskId",
        to = "Column::Id"
    )]
    RetryOfTask,
    #[sea_orm(
        belongs_to = "super::schools::Entity",
        from = "Column::SchoolId",
        to = "super::schools::Column::Id"
    )]
    School,
    #[sea_orm(
        belongs_to = "super::teachers::Entity",
        from = "Column::TeacherId",
        to = "super::teachers::Column::Id"
    )]
    Teacher,
    #[sea_orm(
        belongs_to = "super::storybooks::Entity",
        from = "Column::StorybookId",
        to = "super::storybooks::Column::Id"
    )]
    Storybook,
    #[sea_orm(
        belongs_to = "super::storybook_pages::Entity",
        from = "Column::StorybookPageId",
        to = "super::storybook_pages::Column::Id"
    )]
    StorybookPage,
    #[sea_orm(
        belongs_to = "super::character_profiles::Entity",
        from = "Column::CharacterProfileId",
        to = "super::character_profiles::Column::Id"
    )]
    CharacterProfile,
    #[sea_orm(
        belongs_to = "super::reference_images::Entity",
        from = "Column::ReferenceImageId",
        to = "super::reference_images::Column::Id"
    )]
    ReferenceImage,
    #[sea_orm(has_many = "super::generation_cost_logs::Entity")]
    GenerationCostLogs,
    #[sea_orm(has_many = "super::image_generation_outputs::Entity")]
    ImageGenerationOutputs,
    #[sea_orm(has_many = "super::image_review_events::Entity")]
    ImageReviewEvents,
}

impl Related<super::schools::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::School.def()
    }
}

impl Related<super::teachers::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Teacher.def()
    }
}

impl Related<super::storybooks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Storybook.def()
    }
}

impl Related<super::storybook_pages::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::StorybookPage.def()
    }
}

impl Related<super::character_profiles::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CharacterProfile.def()
    }
}

impl Related<super::reference_images::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ReferenceImage.def()
    }
}

impl Related<super::generation_cost_logs::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::GenerationCostLogs.def()
    }
}

impl Related<super::image_generation_outputs::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ImageGenerationOutputs.def()
    }
}

impl Related<super::image_review_events::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ImageReviewEvents.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
