pub use sea_orm_migration::prelude::*;

mod m20260717_000001_initial_schema;
mod m20260718_000002_create_parent_intakes;
mod m20260718_000003_scope_parent_intakes;
mod m20260718_000004_add_core_query_indexes;
mod m20260718_000005_add_generation_job_retry_metadata;
mod m20260718_000006_add_generation_job_worker_locks;
mod m20260718_000007_add_marketplace_template_source_storybook;
mod m20260718_000008_unique_marketplace_submission_source;
mod m20260718_000009_create_parent_intake_links;
mod m20260719_000010_add_parent_intake_link_access_stats;
mod m20260719_000011_add_delivery_query_indexes;
mod m20260719_000012_parent_intake_classroom_scope;
mod m20260720_000013_create_generation_cost_logs;
mod m20260721_000014_add_export_job_last_error;
mod m20260721_000015_add_share_link_access_stats;
mod m20260722_000016_add_storybook_role_reference_images;
mod m20260724_000017_add_storage_owner_to_jobs;
mod m20260726_000018_add_storybook_teacher_review;
mod m20260727_000019_create_storybook_image_variants;
mod m20260810_000020_add_storybook_page_aspect_ratio;
mod m20260814_000021_create_storybook_creation_sessions;
mod m20260821_000022_create_storybook_creation_assets;
mod m20260821_000023_add_storybook_customization_plan;
mod m20260821_000024_create_storybook_customization_runs;
mod m20260821_000025_add_storybook_page_review_status;
mod m20260824_000026_add_creative_setting_ids;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260717_000001_initial_schema::Migration),
            Box::new(m20260718_000002_create_parent_intakes::Migration),
            Box::new(m20260718_000003_scope_parent_intakes::Migration),
            Box::new(m20260718_000004_add_core_query_indexes::Migration),
            Box::new(m20260718_000005_add_generation_job_retry_metadata::Migration),
            Box::new(m20260718_000006_add_generation_job_worker_locks::Migration),
            Box::new(m20260718_000007_add_marketplace_template_source_storybook::Migration),
            Box::new(m20260718_000008_unique_marketplace_submission_source::Migration),
            Box::new(m20260718_000009_create_parent_intake_links::Migration),
            Box::new(m20260719_000010_add_parent_intake_link_access_stats::Migration),
            Box::new(m20260719_000011_add_delivery_query_indexes::Migration),
            Box::new(m20260719_000012_parent_intake_classroom_scope::Migration),
            Box::new(m20260720_000013_create_generation_cost_logs::Migration),
            Box::new(m20260721_000014_add_export_job_last_error::Migration),
            Box::new(m20260721_000015_add_share_link_access_stats::Migration),
            Box::new(m20260722_000016_add_storybook_role_reference_images::Migration),
            Box::new(m20260724_000017_add_storage_owner_to_jobs::Migration),
            Box::new(m20260726_000018_add_storybook_teacher_review::Migration),
            Box::new(m20260727_000019_create_storybook_image_variants::Migration),
            Box::new(m20260810_000020_add_storybook_page_aspect_ratio::Migration),
            Box::new(m20260814_000021_create_storybook_creation_sessions::Migration),
            Box::new(m20260821_000022_create_storybook_creation_assets::Migration),
            Box::new(m20260821_000023_add_storybook_customization_plan::Migration),
            Box::new(m20260821_000024_create_storybook_customization_runs::Migration),
            Box::new(m20260821_000025_add_storybook_page_review_status::Migration),
            Box::new(m20260824_000026_add_creative_setting_ids::Migration),
        ]
    }
}
