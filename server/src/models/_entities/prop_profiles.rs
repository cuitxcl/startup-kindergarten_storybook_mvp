use sea_orm::entity::prelude::*;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "prop_profiles")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub storybook_id: Option<Uuid>,
    pub child_id: Option<Uuid>,
    pub name: String,
    pub shape: Option<String>,
    pub primary_color: Option<String>,
    pub secondary_color: Option<String>,
    pub material_style: Option<String>,
    pub size_description: Option<String>,
    pub visual_must_keep: Value,
    pub negative_rules: Value,
    pub active_reference_image_id: Option<Uuid>,
    pub status: String,
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
    #[sea_orm(has_many = "super::storybook_page_visual_subjects::Entity")]
    StorybookPageVisualSubjects,
}

impl Related<super::storybooks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Storybook.def()
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

impl Related<super::storybook_page_visual_subjects::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::StorybookPageVisualSubjects.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
