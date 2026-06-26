use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "storybook_page_visual_subjects")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub storybook_page_id: Uuid,
    pub subject_type: String,
    pub storybook_role_id: Option<Uuid>,
    pub prop_profile_id: Option<Uuid>,
    pub importance: String,
    pub placement_hint: Option<String>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::storybook_pages::Entity",
        from = "Column::StorybookPageId",
        to = "super::storybook_pages::Column::Id"
    )]
    StorybookPage,
    #[sea_orm(
        belongs_to = "super::storybook_roles::Entity",
        from = "Column::StorybookRoleId",
        to = "super::storybook_roles::Column::Id"
    )]
    StorybookRole,
    #[sea_orm(
        belongs_to = "super::prop_profiles::Entity",
        from = "Column::PropProfileId",
        to = "super::prop_profiles::Column::Id"
    )]
    PropProfile,
}

impl Related<super::storybook_pages::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::StorybookPage.def()
    }
}

impl Related<super::storybook_roles::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::StorybookRole.def()
    }
}

impl Related<super::prop_profiles::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PropProfile.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
