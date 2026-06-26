use sea_orm::entity::prelude::*;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "storybook_pages")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub storybook_id: Uuid,
    pub page_number: i32,
    pub page_role: String,
    pub page_title: Option<String>,
    pub body_text: String,
    pub prompt_text: Option<String>,
    pub teacher_tip: Option<String>,
    pub scene_spec_json: Option<Value>,
    pub scene_spec_status: String,
    pub page_visual_subjects_json: Option<Value>,
    pub current_image_asset_id: Option<Uuid>,
    pub current_image_task_id: Option<Uuid>,
    pub illustration_status: String,
    pub is_locked: bool,
    pub content_source: String,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::storybooks::Entity",
        from = "Column::StorybookId",
        to = "super::storybooks::Column::Id"
    )]
    Storybook,
    #[sea_orm(
        belongs_to = "super::image_assets::Entity",
        from = "Column::CurrentImageAssetId",
        to = "super::image_assets::Column::Id"
    )]
    CurrentImageAsset,
    #[sea_orm(
        belongs_to = "super::image_generation_tasks::Entity",
        from = "Column::CurrentImageTaskId",
        to = "super::image_generation_tasks::Column::Id"
    )]
    CurrentImageTask,
    #[sea_orm(has_many = "super::image_generation_tasks::Entity")]
    ImageGenerationTasks,
    #[sea_orm(has_many = "super::storybook_page_visual_subjects::Entity")]
    StorybookPageVisualSubjects,
    #[sea_orm(has_many = "super::generation_cost_logs::Entity")]
    GenerationCostLogs,
}

impl Related<super::storybooks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Storybook.def()
    }
}

impl Related<super::image_assets::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CurrentImageAsset.def()
    }
}

impl Related<super::image_generation_tasks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ImageGenerationTasks.def()
    }
}

impl Related<super::storybook_page_visual_subjects::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::StorybookPageVisualSubjects.def()
    }
}

impl Related<super::generation_cost_logs::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::GenerationCostLogs.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
