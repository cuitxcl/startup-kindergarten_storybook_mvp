use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "storybook_roles")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub storybook_id: Uuid,
    pub role_key: String,
    pub role_type: String,
    pub display_name: String,
    pub child_id: Option<Uuid>,
    pub character_profile_id: Option<Uuid>,
    pub parent_character_profile_id: Option<Uuid>,
    pub prop_profile_id: Option<Uuid>,
    pub replacement_source_role_id: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::storybooks::Entity",
        from = "Column::StorybookId",
        to = "super::storybooks::Column::Id"
    )]
    Storybook,
    #[sea_orm(
        belongs_to = "super::children::Entity",
        from = "Column::ChildId",
        to = "super::children::Column::Id"
    )]
    Child,
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
        belongs_to = "super::storybook_roles::Entity",
        from = "Column::ReplacementSourceRoleId",
        to = "Column::Id"
    )]
    ReplacementSourceRole,
    #[sea_orm(has_many = "super::storybook_page_visual_subjects::Entity")]
    StorybookPageVisualSubjects,
}

impl Related<super::storybooks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Storybook.def()
    }
}

impl Related<super::children::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Child.def()
    }
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

impl Related<super::storybook_page_visual_subjects::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::StorybookPageVisualSubjects.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
