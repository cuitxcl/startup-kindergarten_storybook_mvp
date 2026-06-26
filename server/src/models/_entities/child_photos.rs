use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "child_photos")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub child_id: Uuid,
    pub image_asset_id: Uuid,
    pub photo_type: String,
    pub is_primary: bool,
    pub consent_status: String,
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
        belongs_to = "super::image_assets::Entity",
        from = "Column::ImageAssetId",
        to = "super::image_assets::Column::Id"
    )]
    ImageAsset,
    #[sea_orm(has_many = "super::character_profiles::Entity")]
    CharacterProfiles,
}

impl Related<super::children::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Child.def()
    }
}

impl Related<super::image_assets::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ImageAsset.def()
    }
}

impl Related<super::character_profiles::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CharacterProfiles.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
