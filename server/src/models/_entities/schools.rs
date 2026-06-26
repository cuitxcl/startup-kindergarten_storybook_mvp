use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "schools")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub name: String,
    pub code: Option<String>,
    pub status: String,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::classrooms::Entity")]
    Classrooms,
    #[sea_orm(has_many = "super::teachers::Entity")]
    Teachers,
    #[sea_orm(has_many = "super::children::Entity")]
    Children,
    #[sea_orm(has_many = "super::storybooks::Entity")]
    Storybooks,
    #[sea_orm(has_many = "super::image_generation_tasks::Entity")]
    ImageGenerationTasks,
    #[sea_orm(has_many = "super::generation_cost_logs::Entity")]
    GenerationCostLogs,
}

impl Related<super::classrooms::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Classrooms.def()
    }
}

impl Related<super::teachers::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Teachers.def()
    }
}

impl Related<super::children::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Children.def()
    }
}

impl Related<super::storybooks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Storybooks.def()
    }
}

impl Related<super::image_generation_tasks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ImageGenerationTasks.def()
    }
}

impl Related<super::generation_cost_logs::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::GenerationCostLogs.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
