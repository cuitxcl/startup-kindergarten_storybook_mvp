use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "image_generation_outputs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub task_id: Uuid,
    pub image_asset_id: Uuid,
    pub candidate_index: i32,
    pub is_selected: bool,
    pub review_status: String,
    pub quality_notes: Option<String>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::image_generation_tasks::Entity",
        from = "Column::TaskId",
        to = "super::image_generation_tasks::Column::Id"
    )]
    ImageGenerationTask,
    #[sea_orm(
        belongs_to = "super::image_assets::Entity",
        from = "Column::ImageAssetId",
        to = "super::image_assets::Column::Id"
    )]
    ImageAsset,
}

impl Related<super::image_generation_tasks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ImageGenerationTask.def()
    }
}

impl Related<super::image_assets::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ImageAsset.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
