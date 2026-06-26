use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "storybook_share_links")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub storybook_id: Uuid,
    pub share_scope: String,
    pub token_hash: String,
    pub qrcode_asset_id: Option<Uuid>,
    pub anonymize_child_name: bool,
    pub anonymize_parent_info: bool,
    pub status: String,
    pub created_by: Uuid,
    pub created_at: DateTimeWithTimeZone,
    pub expires_at: Option<DateTimeWithTimeZone>,
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
        from = "Column::QrcodeAssetId",
        to = "super::image_assets::Column::Id"
    )]
    QrcodeAsset,
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
        Relation::QrcodeAsset.def()
    }
}

impl Related<super::teachers::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Teacher.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
