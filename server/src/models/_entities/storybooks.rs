use sea_orm::entity::prelude::*;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "storybooks")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub child_id: Option<Uuid>,
    pub teacher_id: Uuid,
    pub template_id: Option<Uuid>,
    pub character_profile_id: Option<Uuid>,
    pub character_profile_version: Option<i32>,
    pub role_manifest_json: Value,
    pub title: String,
    pub content_type: String,
    pub theme: String,
    pub teaching_goal: Option<String>,
    pub style_id: Option<String>,
    pub reading_age_group: Option<String>,
    pub status: String,
    pub export_status: String,
    pub share_status: String,
    pub share_scope: String,
    pub source_template_id: Option<Uuid>,
    pub source_storybook_id: Option<Uuid>,
    pub derivation_type: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::children::Entity",
        from = "Column::ChildId",
        to = "super::children::Column::Id"
    )]
    Child,
    #[sea_orm(
        belongs_to = "super::teachers::Entity",
        from = "Column::TeacherId",
        to = "super::teachers::Column::Id"
    )]
    Teacher,
    #[sea_orm(
        belongs_to = "super::story_templates::Entity",
        from = "Column::TemplateId",
        to = "super::story_templates::Column::Id"
    )]
    StoryTemplate,
    #[sea_orm(
        belongs_to = "super::character_profiles::Entity",
        from = "Column::CharacterProfileId",
        to = "super::character_profiles::Column::Id"
    )]
    CharacterProfile,
    #[sea_orm(has_many = "super::storybook_pages::Entity")]
    StorybookPages,
    #[sea_orm(has_many = "super::image_generation_tasks::Entity")]
    ImageGenerationTasks,
}

impl Related<super::children::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Child.def()
    }
}

impl Related<super::teachers::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Teacher.def()
    }
}

impl Related<super::story_templates::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::StoryTemplate.def()
    }
}

impl Related<super::character_profiles::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CharacterProfile.def()
    }
}

impl Related<super::storybook_pages::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::StorybookPages.def()
    }
}

impl Related<super::image_generation_tasks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ImageGenerationTasks.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
