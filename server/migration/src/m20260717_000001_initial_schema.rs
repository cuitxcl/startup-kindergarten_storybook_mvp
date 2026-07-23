use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        create_users(manager).await?;
        create_workspaces(manager).await?;
        create_workspace_members(manager).await?;
        create_classrooms(manager).await?;
        create_children(manager).await?;
        create_storybooks(manager).await?;
        create_storybook_pages(manager).await?;
        create_storybook_roles(manager).await?;
        create_marketplace_templates(manager).await?;
        create_marketplace_submissions(manager).await?;
        create_share_links(manager).await?;
        create_export_jobs(manager).await?;
        create_generation_jobs(manager).await?;
        create_audit_logs(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AuditLogs::Table).if_exists().to_owned())
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(GenerationJobs::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(ExportJobs::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(ShareLinks::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(MarketplaceSubmissions::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(MarketplaceTemplates::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(StorybookRoles::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(StorybookPages::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(Storybooks::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(Children::Table).if_exists().to_owned())
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(Classrooms::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(WorkspaceMembers::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(Workspaces::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(Users::Table).if_exists().to_owned())
            .await?;
        Ok(())
    }
}

async fn create_users(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Users::Table)
                .if_not_exists()
                .col(uuid_pk(Users::Id))
                .col(string(Users::DisplayName))
                .col(string(Users::Email))
                .col(string_null(Users::PasswordHash))
                .col(string_default(Users::Status, "active"))
                .col(ts(Users::CreatedAt))
                .col(ts(Users::UpdatedAt))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("uidx_users_email")
                .table(Users::Table)
                .col(Users::Email)
                .unique()
                .to_owned(),
        )
        .await
}

async fn create_workspaces(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Workspaces::Table)
                .if_not_exists()
                .col(uuid_pk(Workspaces::Id))
                .col(string(Workspaces::Name))
                .col(string(Workspaces::WorkspaceType))
                .col(text_null(Workspaces::Description))
                .col(string_default(Workspaces::Status, "active"))
                .col(ts(Workspaces::CreatedAt))
                .col(ts(Workspaces::UpdatedAt))
                .to_owned(),
        )
        .await
}

async fn create_workspace_members(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(WorkspaceMembers::Table)
                .if_not_exists()
                .col(uuid_pk(WorkspaceMembers::Id))
                .col(uuid(WorkspaceMembers::WorkspaceId))
                .col(uuid(WorkspaceMembers::UserId))
                .col(string(WorkspaceMembers::Role))
                .col(string_default(WorkspaceMembers::Status, "active"))
                .col(json(WorkspaceMembers::ClassroomIds))
                .col(ts(WorkspaceMembers::CreatedAt))
                .col(ts(WorkspaceMembers::UpdatedAt))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("uidx_workspace_members_scope")
                .table(WorkspaceMembers::Table)
                .col(WorkspaceMembers::WorkspaceId)
                .col(WorkspaceMembers::UserId)
                .unique()
                .to_owned(),
        )
        .await
}

async fn create_classrooms(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Classrooms::Table)
                .if_not_exists()
                .col(uuid_pk(Classrooms::Id))
                .col(uuid(Classrooms::WorkspaceId))
                .col(string(Classrooms::Name))
                .col(string_null(Classrooms::AgeGroup))
                .col(string_default(Classrooms::Status, "active"))
                .col(ts(Classrooms::CreatedAt))
                .col(ts(Classrooms::UpdatedAt))
                .to_owned(),
        )
        .await
}

async fn create_children(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Children::Table)
                .if_not_exists()
                .col(uuid_pk(Children::Id))
                .col(uuid(Children::WorkspaceId))
                .col(uuid_null(Children::ClassroomId))
                .col(string(Children::Nickname))
                .col(string(Children::AgeGroup))
                .col(json(Children::Interests))
                .col(json(Children::Traits))
                .col(text(Children::Focus))
                .col(integer(Children::Completeness))
                .col(string_default(Children::Status, "active"))
                .col(ts(Children::CreatedAt))
                .col(ts(Children::UpdatedAt))
                .to_owned(),
        )
        .await
}

async fn create_storybooks(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Storybooks::Table)
                .if_not_exists()
                .col(uuid_pk(Storybooks::Id))
                .col(uuid(Storybooks::WorkspaceId))
                .col(string(Storybooks::StorybookType))
                .col(string_default(Storybooks::Status, "draft"))
                .col(string_default(Storybooks::Visibility, "private"))
                .col(string(Storybooks::Source))
                .col(uuid_null(Storybooks::SourceStorybookId))
                .col(uuid_null(Storybooks::TargetChildId))
                .col(string(Storybooks::Title))
                .col(string_null(Storybooks::AgeGroup))
                .col(string_null(Storybooks::UseScene))
                .col(text_null(Storybooks::TeachingGoal))
                .col(string_null(Storybooks::CoverTone))
                .col(uuid_null(Storybooks::CreatorId))
                .col(ts(Storybooks::CreatedAt))
                .col(ts(Storybooks::UpdatedAt))
                .to_owned(),
        )
        .await
}

async fn create_storybook_pages(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(StorybookPages::Table)
                .if_not_exists()
                .col(uuid_pk(StorybookPages::Id))
                .col(uuid(StorybookPages::StorybookId))
                .col(integer(StorybookPages::PageNumber))
                .col(string(StorybookPages::Title))
                .col(text(StorybookPages::Body))
                .col(text(StorybookPages::IllustrationPrompt))
                .col(string_default(StorybookPages::Status, "ready"))
                .to_owned(),
        )
        .await
}

async fn create_storybook_roles(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(StorybookRoles::Table)
                .if_not_exists()
                .col(uuid_pk(StorybookRoles::Id))
                .col(uuid(StorybookRoles::StorybookId))
                .col(string(StorybookRoles::Name))
                .col(string(StorybookRoles::RoleType))
                .col(text(StorybookRoles::Appearance))
                .col(text_null(StorybookRoles::StoryFunction))
                .col(boolean_default(StorybookRoles::NeedsConsistency, true))
                .col(text_null(StorybookRoles::ReferenceImageUrl))
                .col(text_null(StorybookRoles::ReferenceImagePrompt))
                .col(string_default(
                    StorybookRoles::ReferenceStatus,
                    "not_started",
                ))
                .to_owned(),
        )
        .await
}

async fn create_marketplace_templates(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(MarketplaceTemplates::Table)
                .if_not_exists()
                .col(uuid_pk(MarketplaceTemplates::Id))
                .col(string(MarketplaceTemplates::SourceType))
                .col(uuid_null(MarketplaceTemplates::SourceWorkspaceId))
                .col(uuid_null(MarketplaceTemplates::SourceStorybookId))
                .col(string(MarketplaceTemplates::Title))
                .col(text(MarketplaceTemplates::Summary))
                .col(string_null(MarketplaceTemplates::AgeGroup))
                .col(string_null(MarketplaceTemplates::UseScene))
                .col(integer(MarketplaceTemplates::PageCount))
                .col(boolean_default(
                    MarketplaceTemplates::SupportsCustomization,
                    true,
                ))
                .col(json(MarketplaceTemplates::Tags))
                .col(string_default(MarketplaceTemplates::Status, "listed"))
                .to_owned(),
        )
        .await
}

async fn create_marketplace_submissions(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(MarketplaceSubmissions::Table)
                .if_not_exists()
                .col(uuid_pk(MarketplaceSubmissions::Id))
                .col(uuid(MarketplaceSubmissions::WorkspaceId))
                .col(uuid(MarketplaceSubmissions::SourceStorybookId))
                .col(string(MarketplaceSubmissions::Title))
                .col(uuid_null(MarketplaceSubmissions::SubmittedBy))
                .col(string_default(MarketplaceSubmissions::Status, "draft"))
                .col(boolean_default(
                    MarketplaceSubmissions::PrivacyConfirmed,
                    false,
                ))
                .col(ts(MarketplaceSubmissions::UpdatedAt))
                .to_owned(),
        )
        .await
}

async fn create_share_links(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(ShareLinks::Table)
                .if_not_exists()
                .col(uuid_pk(ShareLinks::Id))
                .col(uuid(ShareLinks::StorybookId))
                .col(string(ShareLinks::Token))
                .col(string_default(ShareLinks::Status, "active"))
                .col(integer_default(ShareLinks::AccessCount, 0))
                .col(timestamp_null(ShareLinks::LastAccessedAt))
                .col(ts(ShareLinks::CreatedAt))
                .col(timestamp_null(ShareLinks::ExpiresAt))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("uidx_share_links_token")
                .table(ShareLinks::Table)
                .col(ShareLinks::Token)
                .unique()
                .to_owned(),
        )
        .await
}

async fn create_export_jobs(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(ExportJobs::Table)
                .if_not_exists()
                .col(uuid_pk(ExportJobs::Id))
                .col(uuid(ExportJobs::StorybookId))
                .col(string_default(ExportJobs::Status, "queued"))
                .col(string_null(ExportJobs::FileUrl))
                .col(text_null(ExportJobs::LastError))
                .col(ts(ExportJobs::CreatedAt))
                .col(timestamp_null(ExportJobs::FinishedAt))
                .to_owned(),
        )
        .await
}

async fn create_generation_jobs(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(GenerationJobs::Table)
                .if_not_exists()
                .col(uuid_pk(GenerationJobs::Id))
                .col(uuid(GenerationJobs::WorkspaceId))
                .col(uuid_null(GenerationJobs::StorybookId))
                .col(string(GenerationJobs::JobType))
                .col(string_default(GenerationJobs::Status, "queued"))
                .col(json(GenerationJobs::InputJson))
                .col(json_null(GenerationJobs::OutputJson))
                .col(integer_default(GenerationJobs::AttemptCount, 0))
                .col(text_null(GenerationJobs::LastError))
                .col(timestamp_null(GenerationJobs::NextRunAt))
                .col(text_null(GenerationJobs::LockedBy))
                .col(timestamp_null(GenerationJobs::LockedAt))
                .col(ts(GenerationJobs::CreatedAt))
                .col(timestamp_null(GenerationJobs::FinishedAt))
                .to_owned(),
        )
        .await
}

async fn create_audit_logs(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(AuditLogs::Table)
                .if_not_exists()
                .col(uuid_pk(AuditLogs::Id))
                .col(uuid_null(AuditLogs::WorkspaceId))
                .col(uuid_null(AuditLogs::ActorUserId))
                .col(string(AuditLogs::Action))
                .col(string(AuditLogs::ResourceType))
                .col(uuid_null(AuditLogs::ResourceId))
                .col(json_null(AuditLogs::MetadataJson))
                .col(ts(AuditLogs::CreatedAt))
                .to_owned(),
        )
        .await
}

fn uuid_pk<T: IntoIden>(name: T) -> ColumnDef {
    ColumnDef::new(name)
        .uuid()
        .not_null()
        .primary_key()
        .to_owned()
}

fn uuid<T: IntoIden>(name: T) -> ColumnDef {
    ColumnDef::new(name).uuid().not_null().to_owned()
}

fn uuid_null<T: IntoIden>(name: T) -> ColumnDef {
    ColumnDef::new(name).uuid().null().to_owned()
}

fn string<T: IntoIden>(name: T) -> ColumnDef {
    ColumnDef::new(name).string().not_null().to_owned()
}

fn string_null<T: IntoIden>(name: T) -> ColumnDef {
    ColumnDef::new(name).string().null().to_owned()
}

fn string_default<T: IntoIden>(name: T, default: &str) -> ColumnDef {
    ColumnDef::new(name)
        .string()
        .not_null()
        .default(default)
        .to_owned()
}

fn text<T: IntoIden>(name: T) -> ColumnDef {
    ColumnDef::new(name).text().not_null().to_owned()
}

fn text_null<T: IntoIden>(name: T) -> ColumnDef {
    ColumnDef::new(name).text().null().to_owned()
}

fn integer<T: IntoIden>(name: T) -> ColumnDef {
    ColumnDef::new(name).integer().not_null().to_owned()
}

fn integer_default<T: IntoIden>(name: T, default: i32) -> ColumnDef {
    ColumnDef::new(name)
        .integer()
        .not_null()
        .default(default)
        .to_owned()
}

fn json<T: IntoIden>(name: T) -> ColumnDef {
    ColumnDef::new(name).json_binary().not_null().to_owned()
}

fn json_null<T: IntoIden>(name: T) -> ColumnDef {
    ColumnDef::new(name).json_binary().null().to_owned()
}

fn ts<T: IntoIden>(name: T) -> ColumnDef {
    ColumnDef::new(name)
        .timestamp_with_time_zone()
        .not_null()
        .to_owned()
}

fn timestamp_null<T: IntoIden>(name: T) -> ColumnDef {
    ColumnDef::new(name)
        .timestamp_with_time_zone()
        .null()
        .to_owned()
}

fn boolean_default<T: IntoIden>(name: T, default: bool) -> ColumnDef {
    ColumnDef::new(name)
        .boolean()
        .not_null()
        .default(default)
        .to_owned()
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
    DisplayName,
    Email,
    PasswordHash,
    Status,
    CreatedAt,
    UpdatedAt,
}
#[derive(DeriveIden)]
enum Workspaces {
    Table,
    Id,
    Name,
    WorkspaceType,
    Description,
    Status,
    CreatedAt,
    UpdatedAt,
}
#[derive(DeriveIden)]
enum WorkspaceMembers {
    Table,
    Id,
    WorkspaceId,
    UserId,
    Role,
    Status,
    ClassroomIds,
    CreatedAt,
    UpdatedAt,
}
#[derive(DeriveIden)]
enum Classrooms {
    Table,
    Id,
    WorkspaceId,
    Name,
    AgeGroup,
    Status,
    CreatedAt,
    UpdatedAt,
}
#[derive(DeriveIden)]
enum Children {
    Table,
    Id,
    WorkspaceId,
    ClassroomId,
    Nickname,
    AgeGroup,
    Interests,
    Traits,
    Focus,
    Completeness,
    Status,
    CreatedAt,
    UpdatedAt,
}
#[derive(DeriveIden)]
enum Storybooks {
    Table,
    Id,
    WorkspaceId,
    StorybookType,
    Status,
    Visibility,
    Source,
    SourceStorybookId,
    TargetChildId,
    Title,
    AgeGroup,
    UseScene,
    TeachingGoal,
    CoverTone,
    CreatorId,
    CreatedAt,
    UpdatedAt,
}
#[derive(DeriveIden)]
enum StorybookPages {
    Table,
    Id,
    StorybookId,
    PageNumber,
    Title,
    Body,
    IllustrationPrompt,
    Status,
}
#[derive(DeriveIden)]
enum StorybookRoles {
    Table,
    Id,
    StorybookId,
    Name,
    RoleType,
    Appearance,
    StoryFunction,
    NeedsConsistency,
    ReferenceImageUrl,
    ReferenceImagePrompt,
    ReferenceStatus,
}
#[derive(DeriveIden)]
enum MarketplaceTemplates {
    Table,
    Id,
    SourceType,
    SourceWorkspaceId,
    SourceStorybookId,
    Title,
    Summary,
    AgeGroup,
    UseScene,
    PageCount,
    SupportsCustomization,
    Tags,
    Status,
}
#[derive(DeriveIden)]
enum MarketplaceSubmissions {
    Table,
    Id,
    WorkspaceId,
    SourceStorybookId,
    Title,
    SubmittedBy,
    Status,
    PrivacyConfirmed,
    UpdatedAt,
}
#[derive(DeriveIden)]
enum ShareLinks {
    Table,
    Id,
    StorybookId,
    Token,
    Status,
    AccessCount,
    LastAccessedAt,
    CreatedAt,
    ExpiresAt,
}
#[derive(DeriveIden)]
enum ExportJobs {
    Table,
    Id,
    StorybookId,
    Status,
    FileUrl,
    LastError,
    CreatedAt,
    FinishedAt,
}
#[derive(DeriveIden)]
enum GenerationJobs {
    Table,
    Id,
    WorkspaceId,
    StorybookId,
    JobType,
    Status,
    InputJson,
    OutputJson,
    AttemptCount,
    LastError,
    NextRunAt,
    LockedBy,
    LockedAt,
    CreatedAt,
    FinishedAt,
}
#[derive(DeriveIden)]
enum AuditLogs {
    Table,
    Id,
    WorkspaceId,
    ActorUserId,
    Action,
    ResourceType,
    ResourceId,
    MetadataJson,
    CreatedAt,
}
