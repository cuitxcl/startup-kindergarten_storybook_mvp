use sea_orm::entity::prelude::*;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "children")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub school_id: Option<Uuid>,
    pub classroom_id: Option<Uuid>,
    pub primary_teacher_id: Uuid,
    pub primary_parent_id: Option<Uuid>,
    pub name: String,
    pub nickname: Option<String>,
    pub age: Option<i32>,
    pub age_group: Option<String>,
    pub gender_expression: Option<String>,
    pub hair: Option<String>,
    pub skin_tone: Option<String>,
    pub usual_outfit: Option<String>,
    pub favorite_color: Option<String>,
    pub interest_tags: Value,
    pub teacher_observation_tags: Value,
    pub teaching_focus: Option<String>,
    pub profile_completion_status: String,
    pub status: String,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::schools::Entity",
        from = "Column::SchoolId",
        to = "super::schools::Column::Id"
    )]
    School,
    #[sea_orm(
        belongs_to = "super::classrooms::Entity",
        from = "Column::ClassroomId",
        to = "super::classrooms::Column::Id"
    )]
    Classroom,
    #[sea_orm(
        belongs_to = "super::teachers::Entity",
        from = "Column::PrimaryTeacherId",
        to = "super::teachers::Column::Id"
    )]
    PrimaryTeacher,
    #[sea_orm(
        belongs_to = "super::parents::Entity",
        from = "Column::PrimaryParentId",
        to = "super::parents::Column::Id"
    )]
    PrimaryParent,
    #[sea_orm(has_many = "super::child_photos::Entity")]
    ChildPhotos,
    #[sea_orm(has_many = "super::storybooks::Entity")]
    Storybooks,
    #[sea_orm(has_many = "super::character_profiles::Entity")]
    CharacterProfiles,
    #[sea_orm(has_many = "super::parent_character_profiles::Entity")]
    ParentCharacterProfiles,
    #[sea_orm(has_many = "super::prop_profiles::Entity")]
    PropProfiles,
}

impl Related<super::schools::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::School.def()
    }
}

impl Related<super::classrooms::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Classroom.def()
    }
}

impl Related<super::teachers::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PrimaryTeacher.def()
    }
}

impl Related<super::parents::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PrimaryParent.def()
    }
}

impl Related<super::child_photos::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ChildPhotos.def()
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

impl Related<super::parent_character_profiles::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ParentCharacterProfiles.def()
    }
}

impl Related<super::prop_profiles::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PropProfiles.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
