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
        ]
    }
}
