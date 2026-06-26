use sea_orm::entity::prelude::*;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "story_templates")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub title: String,
    pub content_type: String,
    pub theme: String,
    pub teaching_goal: String,
    pub target_age_group: Option<String>,
    pub page_count: i32,
    pub template_outline_json: Value,
    pub default_role_manifest_json: Value,
    pub status: String,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::storybooks::Entity")]
    Storybooks,
    #[sea_orm(has_many = "super::case_storybooks::Entity")]
    CaseStorybooks,
}

impl Related<super::storybooks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Storybooks.def()
    }
}

impl Related<super::case_storybooks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CaseStorybooks.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
