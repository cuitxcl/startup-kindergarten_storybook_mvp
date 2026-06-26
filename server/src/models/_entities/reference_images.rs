use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "reference_images")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub subject_type: String,
    pub character_profile_id: Option<Uuid>,
    pub parent_character_profile_id: Option<Uuid>,
    pub prop_profile_id: Option<Uuid>,
    pub image_asset_id: Uuid,
    pub source_task_id: Option<Uuid>,
    pub style_id: String,
    pub review_status: String,
    pub is_active: bool,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::character_profiles::Entity",
        from = "Column::CharacterProfileId",
        to = "super::character_profiles::Column::Id"
    )]
    CharacterProfile,
    #[sea_orm(
        belongs_to = "super::parent_character_profiles::Entity",
        from = "Column::ParentCharacterProfileId",
        to = "super::parent_character_profiles::Column::Id"
    )]
    ParentCharacterProfile,
    #[sea_orm(
        belongs_to = "super::prop_profiles::Entity",
        from = "Column::PropProfileId",
        to = "super::prop_profiles::Column::Id"
    )]
    PropProfile,
    #[sea_orm(
        belongs_to = "super::image_assets::Entity",
        from = "Column::ImageAssetId",
        to = "super::image_assets::Column::Id"
    )]
    ImageAsset,
    #[sea_orm(
        belongs_to = "super::image_generation_tasks::Entity",
        from = "Column::SourceTaskId",
        to = "super::image_generation_tasks::Column::Id"
    )]
    SourceTask,
    #[sea_orm(has_many = "super::image_generation_tasks::Entity")]
    ImageGenerationTasks,
}

impl Related<super::character_profiles::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CharacterProfile.def()
    }
}

impl Related<super::parent_character_profiles::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ParentCharacterProfile.def()
    }
}

impl Related<super::prop_profiles::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PropProfile.def()
    }
}

impl Related<super::image_assets::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ImageAsset.def()
    }
}

impl Related<super::image_generation_tasks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ImageGenerationTasks.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
