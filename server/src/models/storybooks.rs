use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorybookContentType {
    PlainStorybook,
    CustomStorybook,
    EmotionStory,
    HomeCommunicationCard,
    GrowthMilestoneBook,
}

impl StorybookContentType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::PlainStorybook => "plain_storybook",
            Self::CustomStorybook => "custom_storybook",
            Self::EmotionStory => "emotion_story",
            Self::HomeCommunicationCard => "home_communication_card",
            Self::GrowthMilestoneBook => "growth_milestone_book",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorybookDerivationType {
    Original,
    FromPlainStorybook,
    FromCustomStorybook,
    FromSharedLibrary,
}

impl StorybookDerivationType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::FromPlainStorybook => "from_plain_storybook",
            Self::FromCustomStorybook => "from_custom_storybook",
            Self::FromSharedLibrary => "from_shared_library",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleReplacementInput {
    pub role_key: String,
    pub child_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoleReplacementMode {
    ReuseFirst,
}

impl RoleReplacementMode {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::ReuseFirst => "reuse_first",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplaceRolesRequest {
    pub replacements: Vec<RoleReplacementInput>,
    pub regeneration_mode: RoleReplacementMode,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplaceRolesResponse {
    pub storybook_id: Uuid,
    pub status: String,
    pub replacement_mode: String,
    pub derived_storybook_id: Uuid,
    pub changed_roles: Vec<String>,
    pub regenerated_pages: Vec<i32>,
}
