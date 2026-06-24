use sea_orm::entity::prelude::*;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "character_profiles")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub child_id: Uuid,
    pub version: i32,
    pub name: String,
    pub nickname: Option<String>,
    pub age_group: String,
    pub gender_expression: Option<String>,
    pub hair: String,
    pub skin_tone: Option<String>,
    pub face_shape: Option<String>,
    pub body_proportion: String,
    pub outfit_top: Option<String>,
    pub outfit_bottom: Option<String>,
    pub shoe: Option<String>,
    pub accessory: Option<String>,
    pub signature_colors: Value,
    pub interest_elements: Value,
    pub visual_must_keep: Value,
    pub negative_rules: Value,
    pub source_photo_id: Option<Uuid>,
    pub reference_image_id: Option<Uuid>,
    pub status: String,
    pub created_by: Uuid,
    pub created_at: DateTimeWithTimeZone,
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
        from = "Column::CreatedBy",
        to = "super::teachers::Column::Id"
    )]
    Teacher,
    #[sea_orm(has_many = "super::storybooks::Entity")]
    Storybooks,
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

impl Related<super::storybooks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Storybooks.def()
    }
}

impl Related<super::image_generation_tasks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ImageGenerationTasks.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
