use sea_orm::entity::prelude::*;
use rust_decimal::Decimal;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "generation_cost_logs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub task_id: Uuid,
    pub provider_name: String,
    pub model_name: String,
    pub input_cost: Decimal,
    pub output_cost: Decimal,
    pub total_cost: Decimal,
    pub currency: String,
    pub billed_units: Option<Value>,
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
}

impl Related<super::image_generation_tasks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ImageGenerationTask.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
