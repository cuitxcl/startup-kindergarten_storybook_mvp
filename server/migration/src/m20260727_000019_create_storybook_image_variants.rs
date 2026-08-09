use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                create table if not exists storybook_image_variants (
                  id uuid primary key,
                  workspace_id uuid not null,
                  storybook_id uuid not null,
                  target_type varchar(32) not null,
                  target_id uuid not null,
                  generation_job_id uuid,
                  image_url text,
                  prompt text,
                  provider varchar(64),
                  status varchar(32) not null default 'generating',
                  failure_reason text,
                  is_selected boolean not null default false,
                  created_at timestamptz not null default now(),
                  updated_at timestamptz not null default now()
                );

                alter table storybook_roles
                  add column if not exists selected_image_variant_id uuid;

                alter table storybook_pages
                  add column if not exists selected_image_variant_id uuid;

                create index if not exists idx_storybook_image_variants_target
                  on storybook_image_variants (workspace_id, storybook_id, target_type, target_id, created_at desc);

                create index if not exists idx_storybook_image_variants_job
                  on storybook_image_variants (generation_job_id);

                insert into storybook_image_variants
                  (id, workspace_id, storybook_id, target_type, target_id, generation_job_id,
                   image_url, prompt, provider, status, failure_reason, is_selected, created_at, updated_at)
                select
                  (substr(md5('role_reference:' || gj.id::text), 1, 8) || '-' ||
                   substr(md5('role_reference:' || gj.id::text), 9, 4) || '-' ||
                   substr(md5('role_reference:' || gj.id::text), 13, 4) || '-' ||
                   substr(md5('role_reference:' || gj.id::text), 17, 4) || '-' ||
                   substr(md5('role_reference:' || gj.id::text), 21, 12))::uuid,
                  gj.workspace_id,
                  gj.storybook_id,
                  'role_reference',
                  (gj.input_json->>'role_id')::uuid,
                  gj.id,
                  gj.output_json #>> '{image,image_url}',
                  coalesce(gj.output_json #>> '{image,prompt}', gj.input_json->>'prompt'),
                  gj.output_json->>'provider',
                  'ready',
                  null,
                  false,
                  gj.created_at,
                  coalesce(gj.finished_at, gj.created_at)
                from generation_jobs gj
                where gj.job_type = 'storybook_role_reference_image'
                  and gj.status = 'succeeded'
                  and gj.storybook_id is not null
                  and gj.input_json->>'role_id' ~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
                  and gj.output_json #>> '{image,image_url}' is not null
                  and not exists (
                    select 1 from storybook_image_variants v where v.generation_job_id = gj.id
                  );

                insert into storybook_image_variants
                  (id, workspace_id, storybook_id, target_type, target_id, generation_job_id,
                   image_url, prompt, provider, status, failure_reason, is_selected, created_at, updated_at)
                select
                  (substr(md5('page_illustration:' || gj.id::text), 1, 8) || '-' ||
                   substr(md5('page_illustration:' || gj.id::text), 9, 4) || '-' ||
                   substr(md5('page_illustration:' || gj.id::text), 13, 4) || '-' ||
                   substr(md5('page_illustration:' || gj.id::text), 17, 4) || '-' ||
                   substr(md5('page_illustration:' || gj.id::text), 21, 12))::uuid,
                  gj.workspace_id,
                  gj.storybook_id,
                  'page_illustration',
                  (gj.input_json->>'page_id')::uuid,
                  gj.id,
                  gj.output_json #>> '{image,image_url}',
                  coalesce(gj.output_json #>> '{image,prompt}', gj.input_json->>'prompt'),
                  gj.output_json->>'provider',
                  'ready',
                  null,
                  false,
                  gj.created_at,
                  coalesce(gj.finished_at, gj.created_at)
                from generation_jobs gj
                where gj.job_type = 'storybook_page_image'
                  and gj.status = 'succeeded'
                  and gj.storybook_id is not null
                  and gj.input_json->>'page_id' ~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
                  and gj.output_json #>> '{image,image_url}' is not null
                  and not exists (
                    select 1 from storybook_image_variants v where v.generation_job_id = gj.id
                  );

                with selected_role_variants as (
                  select distinct on (r.id)
                    r.id as role_id,
                    v.id as variant_id
                  from storybook_roles r
                  join storybooks s on s.id = r.storybook_id
                  join storybook_image_variants v
                    on v.workspace_id = s.workspace_id
                   and v.storybook_id = r.storybook_id
                   and v.target_type = 'role_reference'
                   and v.target_id = r.id
                   and v.status = 'ready'
                   and v.image_url = r.reference_image_url
                  where r.reference_image_url is not null
                  order by r.id, v.updated_at desc, v.created_at desc
                )
                update storybook_image_variants v
                set is_selected = true,
                    updated_at = now()
                from selected_role_variants selected
                where v.id = selected.variant_id;

                with selected_role_variants as (
                  select distinct on (r.id)
                    r.id as role_id,
                    v.id as variant_id
                  from storybook_roles r
                  join storybooks s on s.id = r.storybook_id
                  join storybook_image_variants v
                    on v.workspace_id = s.workspace_id
                   and v.storybook_id = r.storybook_id
                   and v.target_type = 'role_reference'
                   and v.target_id = r.id
                   and v.status = 'ready'
                   and v.image_url = r.reference_image_url
                  where r.reference_image_url is not null
                  order by r.id, v.updated_at desc, v.created_at desc
                )
                update storybook_roles r
                set selected_image_variant_id = selected.variant_id
                from selected_role_variants selected
                where r.id = selected.role_id;

                with selected_page_variants as (
                  select distinct on (p.id)
                    p.id as page_id,
                    v.id as variant_id
                  from storybook_pages p
                  join storybooks s on s.id = p.storybook_id
                  join storybook_image_variants v
                    on v.workspace_id = s.workspace_id
                   and v.storybook_id = p.storybook_id
                   and v.target_type = 'page_illustration'
                   and v.target_id = p.id
                   and v.status = 'ready'
                  order by p.id, v.updated_at desc, v.created_at desc
                )
                update storybook_image_variants v
                set is_selected = true,
                    updated_at = now()
                from selected_page_variants selected
                where v.id = selected.variant_id;

                with selected_page_variants as (
                  select distinct on (p.id)
                    p.id as page_id,
                    v.id as variant_id
                  from storybook_pages p
                  join storybooks s on s.id = p.storybook_id
                  join storybook_image_variants v
                    on v.workspace_id = s.workspace_id
                   and v.storybook_id = p.storybook_id
                   and v.target_type = 'page_illustration'
                   and v.target_id = p.id
                   and v.status = 'ready'
                  order by p.id, v.updated_at desc, v.created_at desc
                )
                update storybook_pages p
                set selected_image_variant_id = selected.variant_id
                from selected_page_variants selected
                where p.id = selected.page_id;

                create unique index if not exists uidx_storybook_image_variants_selected_target
                  on storybook_image_variants (workspace_id, storybook_id, target_type, target_id)
                  where is_selected;
                "#,
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                drop index if exists uidx_storybook_image_variants_selected_target;
                drop index if exists idx_storybook_image_variants_job;
                drop index if exists idx_storybook_image_variants_target;

                alter table storybook_pages
                  drop column if exists selected_image_variant_id;

                alter table storybook_roles
                  drop column if exists selected_image_variant_id;

                drop table if exists storybook_image_variants;
                "#,
            )
            .await?;
        Ok(())
    }
}
