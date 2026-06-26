use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "image_review_events")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub task_id: Uuid,
    pub output_id: Option<Uuid>,
    pub reviewer_teacher_id: Option<Uuid>,
    pub review_action: String,
    pub reason_code: Option<String>,
    pub notes: Option<String>,
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
        belongs_to = "super::image_generation_outputs::Entity",
        from = "Column::OutputId",
        to = "super::image_generation_outputs::Column::Id"
    )]
    ImageGenerationOutput,
    #[sea_orm(
        belongs_to = "super::teachers::Entity",
        from = "Column::ReviewerTeacherId",
        to = "super::teachers::Column::Id"
    )]
    ReviewerTeacher,
}

impl Related<super::image_generation_tasks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ImageGenerationTask.def()
    }
}

impl Related<super::image_generation_outputs::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ImageGenerationOutput.def()
    }
}

impl Related<super::teachers::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ReviewerTeacher.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
