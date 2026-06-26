use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "generation_cost_logs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub task_id: Uuid,
    pub school_id: Option<Uuid>,
    pub teacher_id: Option<Uuid>,
    pub storybook_id: Option<Uuid>,
    pub storybook_page_id: Option<Uuid>,
    pub provider_name: String,
    pub model_name: String,
    pub input_units: Option<Decimal>,
    pub output_units: Option<Decimal>,
    pub input_cost: Decimal,
    pub output_cost: Decimal,
    pub total_cost: Decimal,
    pub currency: String,
    pub billed_units_json: Value,
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
        belongs_to = "super::schools::Entity",
        from = "Column::SchoolId",
        to = "super::schools::Column::Id"
    )]
    School,
    #[sea_orm(
        belongs_to = "super::teachers::Entity",
        from = "Column::TeacherId",
        to = "super::teachers::Column::Id"
    )]
    Teacher,
    #[sea_orm(
        belongs_to = "super::storybooks::Entity",
        from = "Column::StorybookId",
        to = "super::storybooks::Column::Id"
    )]
    Storybook,
    #[sea_orm(
        belongs_to = "super::storybook_pages::Entity",
        from = "Column::StorybookPageId",
        to = "super::storybook_pages::Column::Id"
    )]
    StorybookPage,
}

impl Related<super::image_generation_tasks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ImageGenerationTask.def()
    }
}

impl Related<super::schools::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::School.def()
    }
}

impl Related<super::teachers::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Teacher.def()
    }
}

impl Related<super::storybooks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Storybook.def()
    }
}

impl Related<super::storybook_pages::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::StorybookPage.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
