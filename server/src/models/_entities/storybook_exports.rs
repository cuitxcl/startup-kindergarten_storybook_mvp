use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "storybook_exports")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub storybook_id: Uuid,
    pub export_type: String,
    pub status: String,
    pub asset_id: Option<Uuid>,
    pub failure_reason: Option<String>,
    pub created_by: Uuid,
    pub created_at: DateTimeWithTimeZone,
    pub completed_at: Option<DateTimeWithTimeZone>,
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
        from = "Column::AssetId",
        to = "super::image_assets::Column::Id"
    )]
    Asset,
    #[sea_orm(
        belongs_to = "super::teachers::Entity",
        from = "Column::CreatedBy",
        to = "super::teachers::Column::Id"
    )]
    Teacher,
}

impl Related<super::storybooks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Storybook.def()
    }
}

impl Related<super::image_assets::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Asset.def()
    }
}

impl Related<super::teachers::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Teacher.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
