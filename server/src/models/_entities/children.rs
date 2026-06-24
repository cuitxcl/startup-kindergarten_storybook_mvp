use sea_orm::entity::prelude::*;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "children")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub classroom_id: Option<Uuid>,
    pub primary_teacher_id: Uuid,
    pub name: String,
    pub nickname: Option<String>,
    pub age: Option<i32>,
    pub age_group: Option<String>,
    pub gender_expression: Option<String>,
    pub personality_tags: Value,
    pub interests: Value,
    pub favorite_color: Option<String>,
    pub usual_outfit: Option<String>,
    pub notes: Option<String>,
    pub status: String,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::teachers::Entity",
        from = "Column::PrimaryTeacherId",
        to = "super::teachers::Column::Id"
    )]
    Teacher,
    #[sea_orm(has_many = "super::storybooks::Entity")]
    Storybooks,
    #[sea_orm(has_many = "super::character_profiles::Entity")]
    CharacterProfiles,
}

impl Related<super::teachers::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Teacher.def()
    }
}

impl Related<super::storybooks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Storybooks.def()
    }
}

impl Related<super::character_profiles::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CharacterProfiles.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
