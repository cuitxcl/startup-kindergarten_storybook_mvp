use sea_orm::entity::prelude::*;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "image_generation_tasks")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub task_type: String,
    pub parent_task_id: Option<Uuid>,
    pub storybook_id: Option<Uuid>,
    pub page_id: Option<Uuid>,
    pub character_profile_id: Uuid,
    pub character_profile_version: i32,
    pub reference_image_id: Option<Uuid>,
    pub style_id: String,
    pub scene_spec_json: Option<Value>,
    pub prompt_template_version: String,
    pub provider_name: Option<String>,
    pub model_name: Option<String>,
    pub status: String,
    pub retry_count: i32,
    pub failure_reason: Option<String>,
    pub raw_prompt_text: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub completed_at: Option<DateTimeWithTimeZone>,
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
        belongs_to = "super::storybooks::Entity",
        from = "Column::StorybookId",
        to = "super::storybooks::Column::Id"
    )]
    Storybook,
    #[sea_orm(
        belongs_to = "super::storybook_pages::Entity",
        from = "Column::PageId",
        to = "super::storybook_pages::Column::Id"
    )]
    StorybookPage,
    #[sea_orm(
        belongs_to = "super::character_profiles::Entity",
        from = "Column::CharacterProfileId",
        to = "super::character_profiles::Column::Id"
    )]
    CharacterProfile,
    #[sea_orm(has_many = "super::generation_cost_logs::Entity")]
    GenerationCostLogs,
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

impl Related<super::generation_cost_logs::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::GenerationCostLogs.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
