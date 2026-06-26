use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Teachers::Table)
                    .if_not_exists()
                    .col(uuid_pk(Teachers::Id))
                    .col(uuid_null(Teachers::SchoolId))
                    .col(string(Teachers::Name))
                    .col(string_null(Teachers::Email))
                    .col(string_null(Teachers::Phone))
                    .col(string_default(Teachers::Role, "teacher"))
                    .col(string_default(Teachers::Status, "active"))
                    .col(timestamp_with_time_zone(Teachers::CreatedAt))
                    .col(timestamp_with_time_zone(Teachers::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Children::Table)
                    .if_not_exists()
                    .col(uuid_pk(Children::Id))
                    .col(uuid_null(Children::ClassroomId))
                    .col(uuid(Children::PrimaryTeacherId))
                    .col(string(Children::Name))
                    .col(string_null(Children::Nickname))
                    .col(integer_null(Children::Age))
                    .col(string_null(Children::AgeGroup))
                    .col(string_null(Children::GenderExpression))
                    .col(json_not_null(Children::PersonalityTags))
                    .col(json_not_null(Children::Interests))
                    .col(string_null(Children::FavoriteColor))
                    .col(string_null(Children::UsualOutfit))
                    .col(text_null(Children::Notes))
                    .col(string_default(Children::Status, "active"))
                    .col(timestamp_with_time_zone(Children::CreatedAt))
                    .col(timestamp_with_time_zone(Children::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_children_primary_teacher")
                            .from(Children::Table, Children::PrimaryTeacherId)
                            .to(Teachers::Table, Teachers::Id),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_children_primary_teacher_id")
                    .table(Children::Table)
                    .col(Children::PrimaryTeacherId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_children_classroom_id")
                    .table(Children::Table)
                    .col(Children::ClassroomId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(StoryTemplates::Table)
                    .if_not_exists()
                    .col(uuid_pk(StoryTemplates::Id))
                    .col(string(StoryTemplates::Title))
                    .col(string_null(StoryTemplates::DisplayName))
                    .col(string(StoryTemplates::ContentType))
                    .col(string(StoryTemplates::Theme))
                    .col(string(StoryTemplates::TeachingGoal))
                    .col(string_null(StoryTemplates::TargetAgeGroup))
                    .col(integer(StoryTemplates::PageCount))
                    .col(json_not_null(StoryTemplates::StructureJson))
                    .col(boolean_default(StoryTemplates::IsInternalOnly, true))
                    .col(string_default(StoryTemplates::Status, "draft"))
                    .col(timestamp_with_time_zone(StoryTemplates::CreatedAt))
                    .col(timestamp_with_time_zone(StoryTemplates::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ImageAssets::Table)
                    .if_not_exists()
                    .col(uuid_pk(ImageAssets::Id))
                    .col(string(ImageAssets::AssetType))
                    .col(string(ImageAssets::StorageUrl))
                    .col(string_null(ImageAssets::MimeType))
                    .col(integer_null(ImageAssets::Width))
                    .col(integer_null(ImageAssets::Height))
                    .col(big_integer_null(ImageAssets::FileSize))
                    .col(string_null(ImageAssets::Checksum))
                    .col(string_null(ImageAssets::ReviewResult))
                    .col(timestamp_with_time_zone(ImageAssets::CreatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(CharacterProfiles::Table)
                    .if_not_exists()
                    .col(uuid_pk(CharacterProfiles::Id))
                    .col(uuid(CharacterProfiles::ChildId))
                    .col(integer(CharacterProfiles::Version))
                    .col(string(CharacterProfiles::Name))
                    .col(string_null(CharacterProfiles::Nickname))
                    .col(string(CharacterProfiles::AgeGroup))
                    .col(string_null(CharacterProfiles::GenderExpression))
                    .col(string(CharacterProfiles::Hair))
                    .col(string_null(CharacterProfiles::SkinTone))
                    .col(string_null(CharacterProfiles::FaceShape))
                    .col(string(CharacterProfiles::BodyProportion))
                    .col(string_null(CharacterProfiles::OutfitTop))
                    .col(string_null(CharacterProfiles::OutfitBottom))
                    .col(string_null(CharacterProfiles::Shoe))
                    .col(string_null(CharacterProfiles::Accessory))
                    .col(json_not_null(CharacterProfiles::SignatureColors))
                    .col(json_not_null(CharacterProfiles::InterestElements))
                    .col(json_not_null(CharacterProfiles::VisualMustKeep))
                    .col(json_not_null(CharacterProfiles::NegativeRules))
                    .col(uuid_null(CharacterProfiles::SourcePhotoId))
                    .col(uuid_null(CharacterProfiles::ReferenceImageId))
                    .col(string_default(CharacterProfiles::Status, "draft"))
                    .col(uuid(CharacterProfiles::CreatedBy))
                    .col(timestamp_with_time_zone(CharacterProfiles::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_character_profiles_child")
                            .from(CharacterProfiles::Table, CharacterProfiles::ChildId)
                            .to(Children::Table, Children::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_character_profiles_created_by")
                            .from(CharacterProfiles::Table, CharacterProfiles::CreatedBy)
                            .to(Teachers::Table, Teachers::Id),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uidx_character_profiles_child_version")
                    .table(CharacterProfiles::Table)
                    .col(CharacterProfiles::ChildId)
                    .col(CharacterProfiles::Version)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_character_profiles_child_version_desc")
                    .table(CharacterProfiles::Table)
                    .col(CharacterProfiles::ChildId)
                    .col(CharacterProfiles::Version)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Storybooks::Table)
                    .if_not_exists()
                    .col(uuid_pk(Storybooks::Id))
                    .col(uuid_null(Storybooks::ChildId))
                    .col(uuid(Storybooks::TeacherId))
                    .col(uuid_null(Storybooks::TemplateId))
                    .col(uuid_null(Storybooks::CharacterProfileId))
                    .col(integer_null(Storybooks::CharacterProfileVersion))
                    .col(json_not_null(Storybooks::RoleManifestJson))
                    .col(string(Storybooks::Title))
                    .col(string(Storybooks::ContentType))
                    .col(string(Storybooks::Theme))
                    .col(string_null(Storybooks::TeachingGoal))
                    .col(string_null(Storybooks::StyleId))
                    .col(string_null(Storybooks::ReadingAgeGroup))
                    .col(json_object_not_null(Storybooks::GenerationConfigJson))
                    .col(string_default(Storybooks::StoryStatus, "draft"))
                    .col(string_default(
                        Storybooks::IllustrationStatus,
                        "not_started",
                    ))
                    .col(string_default(Storybooks::Status, "draft"))
                    .col(string_default(Storybooks::ExportStatus, "not_exported"))
                    .col(string_default(Storybooks::ShareStatus, "private"))
                    .col(string_default(Storybooks::ShareScope, "private"))
                    .col(uuid_null(Storybooks::SourceTemplateId))
                    .col(uuid_null(Storybooks::SourceStorybookId))
                    .col(string_null(Storybooks::DerivationType))
                    .col(timestamp_with_time_zone(Storybooks::CreatedAt))
                    .col(timestamp_with_time_zone(Storybooks::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_storybooks_child")
                            .from(Storybooks::Table, Storybooks::ChildId)
                            .to(Children::Table, Children::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_storybooks_teacher")
                            .from(Storybooks::Table, Storybooks::TeacherId)
                            .to(Teachers::Table, Teachers::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_storybooks_template")
                            .from(Storybooks::Table, Storybooks::TemplateId)
                            .to(StoryTemplates::Table, StoryTemplates::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_storybooks_character_profile")
                            .from(Storybooks::Table, Storybooks::CharacterProfileId)
                            .to(CharacterProfiles::Table, CharacterProfiles::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_storybooks_source_template")
                            .from(Storybooks::Table, Storybooks::SourceTemplateId)
                            .to(StoryTemplates::Table, StoryTemplates::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_storybooks_source_storybook")
                            .from(Storybooks::Table, Storybooks::SourceStorybookId)
                            .to(Storybooks::Table, Storybooks::Id),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_storybooks_child_created_at")
                    .table(Storybooks::Table)
                    .col(Storybooks::ChildId)
                    .col(Storybooks::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(StorybookPages::Table)
                    .if_not_exists()
                    .col(uuid_pk(StorybookPages::Id))
                    .col(uuid(StorybookPages::StorybookId))
                    .col(integer(StorybookPages::PageNumber))
                    .col(string_default(StorybookPages::PageRole, "story"))
                    .col(string_null(StorybookPages::PageTitle))
                    .col(text(StorybookPages::Body))
                    .col(text_null(StorybookPages::PromptText))
                    .col(text_null(StorybookPages::TeacherTip))
                    .col(json_null(StorybookPages::SceneSpecJson))
                    .col(string_default(StorybookPages::SceneSpecStatus, "missing"))
                    .col(json_null(StorybookPages::PageRolesJson))
                    .col(uuid_null(StorybookPages::ImageAssetId))
                    .col(uuid_null(StorybookPages::CurrentImageTaskId))
                    .col(string_default(
                        StorybookPages::IllustrationStatus,
                        "not_started",
                    ))
                    .col(boolean_default(StorybookPages::IsLocked, false))
                    .col(string_default(StorybookPages::ContentSource, "generated"))
                    .col(timestamp_with_time_zone(StorybookPages::CreatedAt))
                    .col(timestamp_with_time_zone(StorybookPages::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_storybook_pages_storybook")
                            .from(StorybookPages::Table, StorybookPages::StorybookId)
                            .to(Storybooks::Table, Storybooks::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_storybook_pages_image_asset")
                            .from(StorybookPages::Table, StorybookPages::ImageAssetId)
                            .to(ImageAssets::Table, ImageAssets::Id),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uidx_storybook_pages_storybook_page")
                    .table(StorybookPages::Table)
                    .col(StorybookPages::StorybookId)
                    .col(StorybookPages::PageNumber)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ImageGenerationTasks::Table)
                    .if_not_exists()
                    .col(uuid_pk(ImageGenerationTasks::Id))
                    .col(string_null(ImageGenerationTasks::IdempotencyKey))
                    .col(string(ImageGenerationTasks::TaskType))
                    .col(uuid_null(ImageGenerationTasks::ParentTaskId))
                    .col(uuid_null(ImageGenerationTasks::RetryOfTaskId))
                    .col(uuid_null(ImageGenerationTasks::StorybookId))
                    .col(uuid_null(ImageGenerationTasks::PageId))
                    .col(uuid_null(ImageGenerationTasks::CharacterProfileId))
                    .col(integer_null(ImageGenerationTasks::CharacterProfileVersion))
                    .col(uuid_null(ImageGenerationTasks::ReferenceImageId))
                    .col(string(ImageGenerationTasks::StyleId))
                    .col(json_null(ImageGenerationTasks::SceneSpecJson))
                    .col(json_object_not_null(
                        ImageGenerationTasks::InputSnapshotJson,
                    ))
                    .col(string(ImageGenerationTasks::PromptTemplateVersion))
                    .col(string_null(ImageGenerationTasks::ProviderName))
                    .col(string_null(ImageGenerationTasks::ModelName))
                    .col(string_null(ImageGenerationTasks::ProviderRequestId))
                    .col(string_default(ImageGenerationTasks::Status, "queued"))
                    .col(integer_default(ImageGenerationTasks::RetryCount, 0))
                    .col(integer_default(ImageGenerationTasks::MaxRetries, 2))
                    .col(string_null(ImageGenerationTasks::FailureReason))
                    .col(text_null(ImageGenerationTasks::RawPromptText))
                    .col(timestamp_with_time_zone(ImageGenerationTasks::QueuedAt))
                    .col(timestamp_with_time_zone_null(
                        ImageGenerationTasks::StartedAt,
                    ))
                    .col(timestamp_with_time_zone(ImageGenerationTasks::CreatedAt))
                    .col(timestamp_with_time_zone(ImageGenerationTasks::UpdatedAt))
                    .col(timestamp_with_time_zone_null(
                        ImageGenerationTasks::CompletedAt,
                    ))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_image_generation_tasks_parent")
                            .from(
                                ImageGenerationTasks::Table,
                                ImageGenerationTasks::ParentTaskId,
                            )
                            .to(ImageGenerationTasks::Table, ImageGenerationTasks::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_image_generation_tasks_retry_of")
                            .from(
                                ImageGenerationTasks::Table,
                                ImageGenerationTasks::RetryOfTaskId,
                            )
                            .to(ImageGenerationTasks::Table, ImageGenerationTasks::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_image_generation_tasks_storybook")
                            .from(
                                ImageGenerationTasks::Table,
                                ImageGenerationTasks::StorybookId,
                            )
                            .to(Storybooks::Table, Storybooks::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_image_generation_tasks_page")
                            .from(ImageGenerationTasks::Table, ImageGenerationTasks::PageId)
                            .to(StorybookPages::Table, StorybookPages::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_image_generation_tasks_character_profile")
                            .from(
                                ImageGenerationTasks::Table,
                                ImageGenerationTasks::CharacterProfileId,
                            )
                            .to(CharacterProfiles::Table, CharacterProfiles::Id),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uidx_image_generation_tasks_idempotency_key")
                    .table(ImageGenerationTasks::Table)
                    .col(ImageGenerationTasks::IdempotencyKey)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_image_generation_tasks_storybook_status")
                    .table(ImageGenerationTasks::Table)
                    .col(ImageGenerationTasks::StorybookId)
                    .col(ImageGenerationTasks::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_image_generation_tasks_page_status")
                    .table(ImageGenerationTasks::Table)
                    .col(ImageGenerationTasks::PageId)
                    .col(ImageGenerationTasks::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_image_generation_tasks_character_profile_created_at")
                    .table(ImageGenerationTasks::Table)
                    .col(ImageGenerationTasks::CharacterProfileId)
                    .col(ImageGenerationTasks::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ImageGenerationOutputs::Table)
                    .if_not_exists()
                    .col(uuid_pk(ImageGenerationOutputs::Id))
                    .col(uuid(ImageGenerationOutputs::TaskId))
                    .col(uuid(ImageGenerationOutputs::ImageAssetId))
                    .col(integer_default(ImageGenerationOutputs::CandidateIndex, 0))
                    .col(boolean_default(ImageGenerationOutputs::IsSelected, false))
                    .col(string_default(
                        ImageGenerationOutputs::ReviewStatus,
                        "pending",
                    ))
                    .col(text_null(ImageGenerationOutputs::QualityNotes))
                    .col(timestamp_with_time_zone(ImageGenerationOutputs::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_image_generation_outputs_task")
                            .from(
                                ImageGenerationOutputs::Table,
                                ImageGenerationOutputs::TaskId,
                            )
                            .to(ImageGenerationTasks::Table, ImageGenerationTasks::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_image_generation_outputs_image_asset")
                            .from(
                                ImageGenerationOutputs::Table,
                                ImageGenerationOutputs::ImageAssetId,
                            )
                            .to(ImageAssets::Table, ImageAssets::Id),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uidx_image_generation_outputs_task_candidate")
                    .table(ImageGenerationOutputs::Table)
                    .col(ImageGenerationOutputs::TaskId)
                    .col(ImageGenerationOutputs::CandidateIndex)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(GenerationCostLogs::Table)
                    .if_not_exists()
                    .col(uuid_pk(GenerationCostLogs::Id))
                    .col(uuid(GenerationCostLogs::TaskId))
                    .col(uuid_null(GenerationCostLogs::TeacherId))
                    .col(uuid_null(GenerationCostLogs::StorybookId))
                    .col(uuid_null(GenerationCostLogs::PageId))
                    .col(string(GenerationCostLogs::ProviderName))
                    .col(string(GenerationCostLogs::ModelName))
                    .col(decimal_null(GenerationCostLogs::InputUnits))
                    .col(decimal_null(GenerationCostLogs::OutputUnits))
                    .col(decimal(GenerationCostLogs::InputCost))
                    .col(decimal(GenerationCostLogs::OutputCost))
                    .col(decimal(GenerationCostLogs::TotalCost))
                    .col(string(GenerationCostLogs::Currency))
                    .col(json_null(GenerationCostLogs::BilledUnits))
                    .col(timestamp_with_time_zone(GenerationCostLogs::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_generation_cost_logs_task")
                            .from(GenerationCostLogs::Table, GenerationCostLogs::TaskId)
                            .to(ImageGenerationTasks::Table, ImageGenerationTasks::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_generation_cost_logs_teacher")
                            .from(GenerationCostLogs::Table, GenerationCostLogs::TeacherId)
                            .to(Teachers::Table, Teachers::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_generation_cost_logs_storybook")
                            .from(GenerationCostLogs::Table, GenerationCostLogs::StorybookId)
                            .to(Storybooks::Table, Storybooks::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_generation_cost_logs_page")
                            .from(GenerationCostLogs::Table, GenerationCostLogs::PageId)
                            .to(StorybookPages::Table, StorybookPages::Id),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_generation_cost_logs_task_id")
                    .table(GenerationCostLogs::Table)
                    .col(GenerationCostLogs::TaskId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(GenerationCostLogs::Table).to_owned())
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(ImageGenerationOutputs::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(ImageGenerationTasks::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(StorybookPages::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Storybooks::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CharacterProfiles::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ImageAssets::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(StoryTemplates::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Children::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Teachers::Table).to_owned())
            .await?;

        Ok(())
    }
}

fn uuid_pk(column: impl IntoIden) -> ColumnDef {
    ColumnDef::new(column)
        .uuid()
        .not_null()
        .primary_key()
        .default(Expr::cust("gen_random_uuid()"))
        .to_owned()
}

fn uuid(column: impl IntoIden) -> ColumnDef {
    ColumnDef::new(column).uuid().not_null().to_owned()
}

fn uuid_null(column: impl IntoIden) -> ColumnDef {
    ColumnDef::new(column).uuid().null().to_owned()
}

fn string(column: impl IntoIden) -> ColumnDef {
    ColumnDef::new(column).string().not_null().to_owned()
}

fn string_null(column: impl IntoIden) -> ColumnDef {
    ColumnDef::new(column).string().null().to_owned()
}

fn string_default(column: impl IntoIden, value: &str) -> ColumnDef {
    ColumnDef::new(column)
        .string()
        .not_null()
        .default(value)
        .to_owned()
}

fn integer(column: impl IntoIden) -> ColumnDef {
    ColumnDef::new(column).integer().not_null().to_owned()
}

fn integer_null(column: impl IntoIden) -> ColumnDef {
    ColumnDef::new(column).integer().null().to_owned()
}

fn integer_default(column: impl IntoIden, value: i32) -> ColumnDef {
    ColumnDef::new(column)
        .integer()
        .not_null()
        .default(value)
        .to_owned()
}

fn big_integer_null(column: impl IntoIden) -> ColumnDef {
    ColumnDef::new(column).big_integer().null().to_owned()
}

fn boolean_default(column: impl IntoIden, value: bool) -> ColumnDef {
    ColumnDef::new(column)
        .boolean()
        .not_null()
        .default(value)
        .to_owned()
}

fn text(column: impl IntoIden) -> ColumnDef {
    ColumnDef::new(column).text().not_null().to_owned()
}

fn text_null(column: impl IntoIden) -> ColumnDef {
    ColumnDef::new(column).text().null().to_owned()
}

fn json_not_null(column: impl IntoIden) -> ColumnDef {
    ColumnDef::new(column)
        .json_binary()
        .not_null()
        .default(Expr::cust("'[]'::jsonb"))
        .to_owned()
}

fn json_object_not_null(column: impl IntoIden) -> ColumnDef {
    ColumnDef::new(column)
        .json_binary()
        .not_null()
        .default(Expr::cust("'{}'::jsonb"))
        .to_owned()
}

fn json_null(column: impl IntoIden) -> ColumnDef {
    ColumnDef::new(column).json_binary().null().to_owned()
}

fn timestamp_with_time_zone(column: impl IntoIden) -> ColumnDef {
    ColumnDef::new(column)
        .timestamp_with_time_zone()
        .not_null()
        .default(Expr::current_timestamp())
        .to_owned()
}

fn timestamp_with_time_zone_null(column: impl IntoIden) -> ColumnDef {
    ColumnDef::new(column)
        .timestamp_with_time_zone()
        .null()
        .to_owned()
}

fn decimal(column: impl IntoIden) -> ColumnDef {
    ColumnDef::new(column)
        .decimal_len(12, 4)
        .not_null()
        .to_owned()
}

fn decimal_null(column: impl IntoIden) -> ColumnDef {
    ColumnDef::new(column).decimal_len(12, 4).null().to_owned()
}

#[derive(DeriveIden)]
enum Teachers {
    Table,
    Id,
    SchoolId,
    Name,
    Email,
    Phone,
    Role,
    Status,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Children {
    Table,
    Id,
    ClassroomId,
    PrimaryTeacherId,
    Name,
    Nickname,
    Age,
    AgeGroup,
    GenderExpression,
    PersonalityTags,
    Interests,
    FavoriteColor,
    UsualOutfit,
    Notes,
    Status,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum StoryTemplates {
    Table,
    Id,
    Title,
    DisplayName,
    ContentType,
    Theme,
    TeachingGoal,
    TargetAgeGroup,
    PageCount,
    StructureJson,
    IsInternalOnly,
    Status,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Storybooks {
    Table,
    Id,
    ChildId,
    TeacherId,
    TemplateId,
    CharacterProfileId,
    CharacterProfileVersion,
    RoleManifestJson,
    Title,
    ContentType,
    Theme,
    TeachingGoal,
    StyleId,
    ReadingAgeGroup,
    GenerationConfigJson,
    StoryStatus,
    IllustrationStatus,
    Status,
    ExportStatus,
    ShareStatus,
    ShareScope,
    SourceTemplateId,
    SourceStorybookId,
    DerivationType,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum StorybookPages {
    Table,
    Id,
    StorybookId,
    PageNumber,
    PageRole,
    PageTitle,
    Body,
    PromptText,
    TeacherTip,
    SceneSpecJson,
    SceneSpecStatus,
    PageRolesJson,
    ImageAssetId,
    CurrentImageTaskId,
    IllustrationStatus,
    IsLocked,
    ContentSource,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum CharacterProfiles {
    Table,
    Id,
    ChildId,
    Version,
    Name,
    Nickname,
    AgeGroup,
    GenderExpression,
    Hair,
    SkinTone,
    FaceShape,
    BodyProportion,
    OutfitTop,
    OutfitBottom,
    Shoe,
    Accessory,
    SignatureColors,
    InterestElements,
    VisualMustKeep,
    NegativeRules,
    SourcePhotoId,
    ReferenceImageId,
    Status,
    CreatedBy,
    CreatedAt,
}

#[derive(DeriveIden)]
enum ImageGenerationTasks {
    Table,
    Id,
    IdempotencyKey,
    TaskType,
    ParentTaskId,
    RetryOfTaskId,
    StorybookId,
    PageId,
    CharacterProfileId,
    CharacterProfileVersion,
    ReferenceImageId,
    StyleId,
    SceneSpecJson,
    InputSnapshotJson,
    PromptTemplateVersion,
    ProviderName,
    ModelName,
    ProviderRequestId,
    Status,
    RetryCount,
    MaxRetries,
    FailureReason,
    RawPromptText,
    QueuedAt,
    StartedAt,
    CreatedAt,
    UpdatedAt,
    CompletedAt,
}

#[derive(DeriveIden)]
enum ImageGenerationOutputs {
    Table,
    Id,
    TaskId,
    ImageAssetId,
    CandidateIndex,
    IsSelected,
    ReviewStatus,
    QualityNotes,
    CreatedAt,
}

#[derive(DeriveIden)]
enum ImageAssets {
    Table,
    Id,
    AssetType,
    StorageUrl,
    MimeType,
    Width,
    Height,
    FileSize,
    Checksum,
    ReviewResult,
    CreatedAt,
}

#[derive(DeriveIden)]
enum GenerationCostLogs {
    Table,
    Id,
    TaskId,
    TeacherId,
    StorybookId,
    PageId,
    ProviderName,
    ModelName,
    InputUnits,
    OutputUnits,
    InputCost,
    OutputCost,
    TotalCost,
    Currency,
    BilledUnits,
    CreatedAt,
}
