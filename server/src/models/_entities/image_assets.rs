use sea_orm::entity::prelude::*;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "image_assets")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub asset_type: String,
    pub storage_url: String,
    pub storage_key: Option<String>,
    pub mime_type: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub file_size: Option<i64>,
    pub checksum: Option<String>,
    pub review_result: Option<String>,
    pub metadata_json: Value,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::storybook_pages::Entity")]
    StorybookPages,
    #[sea_orm(has_many = "super::child_photos::Entity")]
    ChildPhotos,
    #[sea_orm(has_many = "super::reference_images::Entity")]
    ReferenceImages,
    #[sea_orm(has_many = "super::image_generation_outputs::Entity")]
    ImageGenerationOutputs,
    #[sea_orm(has_many = "super::case_storybooks::Entity")]
    CaseStorybooks,
    #[sea_orm(has_many = "super::storybook_exports::Entity")]
    StorybookExports,
    #[sea_orm(has_many = "super::storybook_share_links::Entity")]
    StorybookShareLinks,
}

impl Related<super::storybook_pages::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::StorybookPages.def()
    }
}

impl Related<super::child_photos::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ChildPhotos.def()
    }
}

impl Related<super::reference_images::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ReferenceImages.def()
    }
}

impl Related<super::image_generation_outputs::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ImageGenerationOutputs.def()
    }
}

impl Related<super::case_storybooks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CaseStorybooks.def()
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
