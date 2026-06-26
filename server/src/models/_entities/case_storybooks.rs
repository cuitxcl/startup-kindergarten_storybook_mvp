use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "case_storybooks")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub storybook_id: Option<Uuid>,
    pub template_id: Option<Uuid>,
    pub title: String,
    pub theme: String,
    pub teaching_goal: String,
    pub target_age_group: Option<String>,
    pub cover_image_asset_id: Option<Uuid>,
    pub page_count: i32,
    pub status: String,
    pub sort_order: i32,
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
        belongs_to = "super::story_templates::Entity",
        from = "Column::TemplateId",
        to = "super::story_templates::Column::Id"
    )]
    Template,
    #[sea_orm(
        belongs_to = "super::image_assets::Entity",
        from = "Column::CoverImageAssetId",
        to = "super::image_assets::Column::Id"
    )]
    CoverImageAsset,
    #[sea_orm(has_many = "super::storybooks::Entity")]
    Storybooks,
}

impl Related<super::storybooks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Storybooks.def()
    }
}

impl Related<super::story_templates::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Template.def()
    }
}

impl Related<super::image_assets::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CoverImageAsset.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
