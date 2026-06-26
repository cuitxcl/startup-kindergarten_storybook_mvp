use sea_orm::entity::prelude::*;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "storybooks")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub school_id: Option<Uuid>,
    pub teacher_id: Uuid,
    pub child_id: Option<Uuid>,
    pub story_template_id: Option<Uuid>,
    pub case_storybook_id: Option<Uuid>,
    pub source_storybook_id: Option<Uuid>,
    pub title: String,
    pub content_type: String,
    pub theme: String,
    pub teaching_goal: Option<String>,
    pub style_id: Option<String>,
    pub reading_age_group: Option<String>,
    pub generation_config_json: Value,
    pub role_manifest_json: Value,
    pub story_status: String,
    pub illustration_status: String,
    pub status: String,
    pub export_status: String,
    pub share_status: String,
    pub share_scope: String,
    pub derivation_type: String,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub exported_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::schools::Entity",
        from = "Column::SchoolId",
        to = "super::schools::Column::Id"
    )]
    School,
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
        from = "Column::StoryTemplateId",
        to = "super::story_templates::Column::Id"
    )]
    StoryTemplate,
    #[sea_orm(
        belongs_to = "super::case_storybooks::Entity",
        from = "Column::CaseStorybookId",
        to = "super::case_storybooks::Column::Id"
    )]
    CaseStorybook,
    #[sea_orm(has_many = "super::storybook_pages::Entity")]
    StorybookPages,
    #[sea_orm(has_many = "super::storybook_roles::Entity")]
    StorybookRoles,
    #[sea_orm(has_many = "super::prop_profiles::Entity")]
    PropProfiles,
    #[sea_orm(has_many = "super::image_generation_tasks::Entity")]
    ImageGenerationTasks,
    #[sea_orm(has_many = "super::generation_cost_logs::Entity")]
    GenerationCostLogs,
    #[sea_orm(has_many = "super::storybook_exports::Entity")]
    StorybookExports,
    #[sea_orm(has_many = "super::storybook_share_links::Entity")]
    StorybookShareLinks,
}

impl Related<super::schools::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::School.def()
    }
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

impl Related<super::case_storybooks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CaseStorybook.def()
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

impl Related<super::storybook_roles::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::StorybookRoles.def()
    }
}

impl Related<super::prop_profiles::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PropProfiles.def()
    }
}

impl Related<super::generation_cost_logs::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::GenerationCostLogs.def()
    }
}

impl Related<super::storybook_exports::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::StorybookExports.def()
    }
}

impl Related<super::storybook_share_links::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::StorybookShareLinks.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
