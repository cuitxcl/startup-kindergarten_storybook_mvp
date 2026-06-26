use sea_orm::entity::prelude::*;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "parent_character_profiles")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub parent_id: Uuid,
    pub child_id: Option<Uuid>,
    pub version: i32,
    pub role: String,
    pub name: String,
    pub hair: Option<String>,
    pub skin_tone: Option<String>,
    pub face_shape: Option<String>,
    pub body_proportion: Option<String>,
    pub outfit_top: Option<String>,
    pub outfit_bottom: Option<String>,
    pub accessory: Option<String>,
    pub visual_must_keep: Value,
    pub negative_rules: Value,
    pub active_reference_image_id: Option<Uuid>,
    pub status: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::parents::Entity",
        from = "Column::ParentId",
        to = "super::parents::Column::Id"
    )]
    Parent,
    #[sea_orm(
        belongs_to = "super::children::Entity",
        from = "Column::ChildId",
        to = "super::children::Column::Id"
    )]
    Child,
    #[sea_orm(
        belongs_to = "super::reference_images::Entity",
        from = "Column::ActiveReferenceImageId",
        to = "super::reference_images::Column::Id"
    )]
    ActiveReferenceImage,
    #[sea_orm(has_many = "super::reference_images::Entity")]
    ReferenceImages,
    #[sea_orm(has_many = "super::storybook_roles::Entity")]
    StorybookRoles,
}

impl Related<super::parents::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Parent.def()
    }
}

impl Related<super::children::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Child.def()
    }
}

impl Related<super::reference_images::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ReferenceImages.def()
    }
}

impl Related<super::storybook_roles::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::StorybookRoles.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
