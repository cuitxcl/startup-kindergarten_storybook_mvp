use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "parents")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub name: String,
    pub relationship_to_child: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub status: String,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::children::Entity")]
    Children,
    #[sea_orm(has_many = "super::parent_character_profiles::Entity")]
    ParentCharacterProfiles,
}

impl Related<super::children::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Children.def()
    }
}

impl Related<super::parent_character_profiles::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ParentCharacterProfiles.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
