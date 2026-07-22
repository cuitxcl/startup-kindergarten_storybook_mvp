use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ParentIntakes::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ParentIntakes::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ParentIntakes::ChildNickname)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ParentIntakes::AgeGroup).string().not_null())
                    .col(
                        ColumnDef::new(ParentIntakes::Interests)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ParentIntakes::Status)
                            .string()
                            .not_null()
                            .default("submitted"),
                    )
                    .col(
                        ColumnDef::new(ParentIntakes::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(ParentIntakes::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum ParentIntakes {
    Table,
    Id,
    ChildNickname,
    AgeGroup,
    Interests,
    Status,
    CreatedAt,
}
