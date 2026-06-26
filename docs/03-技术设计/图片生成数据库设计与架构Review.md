# 图片生成数据库设计与架构 Review

## 设计结论

本文是在 `产品需求文档.md`、图片生成相关设计文档、现有 `数据模型设计.md`、`完整版数据库结构设计.md` 和当前 SeaORM migration 基础上重新整理的数据库设计。重点覆盖图片生成、角色一致性、页面重绘、成本追踪和后续架构 review 后的修订意见。

Phase 1 不建议把图片生成当成 `storybook_pages.image_asset_id` 的附属字段处理。图片生成是独立生产链路，应有可追踪任务、输入快照、输出资产、审核结果、成本记录和重试血缘。

## Domain summary

产品核心闭环是：

1. 家长或老师创建儿童档案。
2. 老师选择成品案例或普通绘本母本，先生成故事内容。
3. 系统先落库 `storybooks` 和 `storybook_pages`，页面正文、互动提问、页面角色和基础 `scene_spec_json` 成为后续图片生成输入。
4. 对定制绘本，系统再规范化儿童/家长角色卡；对普通绘本，可使用默认角色或仅使用故事页场景结构。
5. 按 `02-图片生成/图片生成稳定性方案.md`，先固定角色、道具和画风锚点，再为分页插图创建图片任务。
6. 老师可以逐页编辑、锁定、单页重写、单页重绘。
7. 读本可导出 PDF、电子翻页版、分享链接和二维码。
8. 园所需要追踪图片生成成本、失败率、重试率和审核风险。

图片生成稳定性的核心数据原则：

- 故事文本和页面结构先生成，图片生成不能成为故事落库的前置条件。
- 角色、道具、画风是图片阶段常量，页面场景来自已生成的 `storybook_pages.scene_spec_json`。
- 任务发起时必须固化输入快照，不能只引用会变的当前角色卡。
- 单页任务必须可独立失败、重试、审核和替换生效图。
- 成本必须能按任务、读本、老师、园所、模型维度追溯。
- 读本页面当前生效图与历史生成结果要分开建模。

## Entities

### 组织与用户域

- `schools`
- `classrooms`
- `teachers`
- `parents`
- `children`
- `child_photos`

### 内容域

- `story_templates`
- `case_storybooks`
- `storybooks`
- `storybook_pages`
- `storybook_exports`
- `storybook_share_links`

### 角色与一致性域

- `character_profiles`
- `parent_character_profiles`
- `prop_profiles`
- `reference_images`
- `storybook_roles`
- `storybook_page_roles`

### 图片生成域

- `image_assets`
- `image_generation_tasks`
- `image_generation_outputs`
- `image_review_events`
- `generation_cost_logs`

## Fields by entity

### `schools`

- `id`: uuid, primary key
- `name`: varchar, not null
- `code`: varchar, nullable, unique when present
- `status`: varchar, not null, enum `active` / `inactive`
- `created_at`: timestamptz, not null
- `updated_at`: timestamptz, not null

### `classrooms`

- `id`: uuid, primary key
- `school_id`: uuid, not null, FK `schools.id`
- `teacher_id`: uuid, nullable, FK `teachers.id`
- `name`: varchar, not null
- `grade_level`: varchar, nullable
- `status`: varchar, not null, enum `active` / `archived`
- `created_at`: timestamptz, not null
- `updated_at`: timestamptz, not null

### `teachers`

- `id`: uuid, primary key
- `school_id`: uuid, nullable, FK `schools.id`
- `name`: varchar, not null
- `email`: varchar, nullable
- `phone`: varchar, nullable
- `role`: varchar, not null, enum `teacher` / `school_admin` / `operator`
- `status`: varchar, not null, enum `active` / `inactive`
- `created_at`: timestamptz, not null
- `updated_at`: timestamptz, not null

### `parents`

- `id`: uuid, primary key
- `name`: varchar, not null
- `relationship_to_child`: varchar, nullable
- `phone`: varchar, nullable
- `email`: varchar, nullable
- `status`: varchar, not null, enum `active` / `inactive`
- `created_at`: timestamptz, not null
- `updated_at`: timestamptz, not null

### `children`

- `id`: uuid, primary key
- `school_id`: uuid, nullable, FK `schools.id`
- `classroom_id`: uuid, nullable, FK `classrooms.id`
- `primary_teacher_id`: uuid, not null, FK `teachers.id`
- `primary_parent_id`: uuid, nullable, FK `parents.id`
- `name`: varchar, not null
- `nickname`: varchar, nullable
- `age`: integer, nullable
- `age_group`: varchar, nullable
- `gender_expression`: varchar, nullable
- `hair`: varchar, nullable
- `skin_tone`: varchar, nullable
- `usual_outfit`: varchar, nullable
- `favorite_color`: varchar, nullable
- `interest_tags`: jsonb, not null, default `[]`
- `teacher_observation_tags`: jsonb, not null, default `[]`
- `teaching_focus`: text, nullable
- `profile_completion_status`: varchar, not null, enum `missing_required` / `usable` / `complete`
- `status`: varchar, not null, enum `active` / `archived`
- `created_at`: timestamptz, not null
- `updated_at`: timestamptz, not null

### `child_photos`

- `id`: uuid, primary key
- `child_id`: uuid, not null, FK `children.id`
- `image_asset_id`: uuid, not null, FK `image_assets.id`
- `photo_type`: varchar, not null, enum `portrait` / `daily` / `other`
- `is_primary`: boolean, not null, default `false`
- `consent_status`: varchar, not null, enum `pending` / `granted` / `revoked`
- `created_at`: timestamptz, not null

### `story_templates`

`story_templates` 是后台维护的结构骨架，不是老师前台优先看到的成品案例。

- `id`: uuid, primary key
- `title`: varchar, not null
- `content_type`: varchar, not null, enum `plain_storybook` / `custom_storybook`
- `theme`: varchar, not null
- `teaching_goal`: text, not null
- `target_age_group`: varchar, nullable
- `page_count`: integer, not null
- `template_outline_json`: jsonb, not null
- `default_role_manifest_json`: jsonb, not null, default `{}`
- `status`: varchar, not null, enum `draft` / `active` / `archived`
- `created_at`: timestamptz, not null
- `updated_at`: timestamptz, not null

### `case_storybooks`

成品绘本案例库应直接展示成品效果。它可以引用一条已完成 `storybooks`，也可以引用运营维护的案例成品。

- `id`: uuid, primary key
- `storybook_id`: uuid, nullable, FK `storybooks.id`
- `template_id`: uuid, nullable, FK `story_templates.id`
- `title`: varchar, not null
- `theme`: varchar, not null
- `teaching_goal`: text, not null
- `target_age_group`: varchar, nullable
- `cover_image_asset_id`: uuid, nullable, FK `image_assets.id`
- `page_count`: integer, not null
- `status`: varchar, not null, enum `draft` / `published` / `archived`
- `sort_order`: integer, not null, default `0`
- `created_at`: timestamptz, not null
- `updated_at`: timestamptz, not null

### `storybooks`

- `id`: uuid, primary key
- `school_id`: uuid, nullable, FK `schools.id`
- `teacher_id`: uuid, not null, FK `teachers.id`
- `child_id`: uuid, nullable, FK `children.id`
- `story_template_id`: uuid, nullable, FK `story_templates.id`
- `case_storybook_id`: uuid, nullable, FK `case_storybooks.id`
- `source_storybook_id`: uuid, nullable, FK `storybooks.id`
- `title`: varchar, not null
- `content_type`: varchar, not null, enum `plain_storybook` / `custom_storybook`
- `theme`: varchar, not null
- `teaching_goal`: text, nullable
- `style_id`: varchar, nullable
- `reading_age_group`: varchar, nullable
- `generation_config_json`: jsonb, not null, default `{}`
- `role_manifest_json`: jsonb, not null, default `{}`
- `story_status`: varchar, not null, enum `draft` / `story_generating` / `story_ready` / `story_failed`
- `illustration_status`: varchar, not null, enum `not_started` / `queued` / `running` / `ready` / `partial_failed` / `failed`
- `status`: varchar, not null, enum `draft` / `generating` / `ready` / `exporting` / `archived`
- `export_status`: varchar, not null, enum `not_exported` / `exporting` / `exported` / `failed`
- `share_status`: varchar, not null, enum `private` / `link_created` / `shared`
- `share_scope`: varchar, not null, enum `private` / `family` / `school` / `platform_review` / `platform_public`
- `derivation_type`: varchar, not null, enum `original` / `from_case` / `from_plain_storybook` / `from_custom_storybook` / `from_shared_library`
- `created_at`: timestamptz, not null
- `updated_at`: timestamptz, not null
- `exported_at`: timestamptz, nullable

Rule:

- `child_id` nullable only when `content_type = plain_storybook`.
- `custom_storybook` should have at least one protagonist entry in `role_manifest_json`.
- `source_storybook_id` records ordinary-book-to-custom-book derivation.
- `story_status = story_ready` is the gate for starting page illustration tasks.
- `status = ready` requires story ready and either illustration ready or the product explicitly allows text-only export.

### `storybook_pages`

- `id`: uuid, primary key
- `storybook_id`: uuid, not null, FK `storybooks.id`
- `page_number`: integer, not null
- `page_role`: varchar, not null, enum `cover` / `story` / `closing`
- `page_title`: varchar, nullable
- `body_text`: text, not null
- `prompt_text`: text, nullable
- `teacher_tip`: text, nullable
- `scene_spec_json`: jsonb, nullable
- `scene_spec_status`: varchar, not null, enum `missing` / `draft` / `ready`
- `page_roles_json`: jsonb, nullable
- `current_image_asset_id`: uuid, nullable, FK `image_assets.id`
- `current_image_task_id`: uuid, nullable, FK `image_generation_tasks.id`
- `illustration_status`: varchar, not null, enum `not_started` / `queued` / `running` / `ready` / `needs_review` / `failed`
- `is_locked`: boolean, not null, default `false`
- `content_source`: varchar, not null, enum `template` / `generated` / `manual_edit`
- `created_at`: timestamptz, not null
- `updated_at`: timestamptz, not null

Rule:

- `(storybook_id, page_number)` unique.
- `current_image_asset_id` only points to the image currently used by the page. Historical generated candidates live in `image_generation_outputs`.
- `scene_spec_status = ready` is required before creating a page image task.
- Text editing can update `body_text` without immediately regenerating image; explicit redraw creates a new image task.

### `character_profiles`

儿童角色卡必须版本化。生成任务不能只引用当前 active 版本，还要保存输入快照。

- `id`: uuid, primary key
- `child_id`: uuid, not null, FK `children.id`
- `version`: integer, not null
- `name`: varchar, not null
- `nickname`: varchar, nullable
- `age_group`: varchar, not null
- `gender_expression`: varchar, nullable
- `hair`: varchar, not null
- `skin_tone`: varchar, nullable
- `face_shape`: varchar, nullable
- `body_proportion`: varchar, not null
- `outfit_top`: varchar, nullable
- `outfit_bottom`: varchar, nullable
- `shoe`: varchar, nullable
- `accessory`: varchar, nullable
- `signature_colors`: jsonb, not null, default `[]`
- `interest_elements`: jsonb, not null, default `[]`
- `visual_must_keep`: jsonb, not null, default `[]`
- `negative_rules`: jsonb, not null, default `[]`
- `source_photo_id`: uuid, nullable, FK `child_photos.id`
- `active_reference_image_id`: uuid, nullable, FK `reference_images.id`
- `status`: varchar, not null, enum `draft` / `active` / `superseded`
- `created_by`: uuid, not null, FK `teachers.id`
- `created_at`: timestamptz, not null

Rule:

- `(child_id, version)` unique.
- Only one active profile per child should be enforced by a partial unique index where supported.
- `visual_must_keep` should have at least three entries before generating reference image.

### `parent_character_profiles`

- `id`: uuid, primary key
- `parent_id`: uuid, not null, FK `parents.id`
- `child_id`: uuid, nullable, FK `children.id`
- `version`: integer, not null
- `role`: varchar, not null
- `name`: varchar, not null
- `hair`: varchar, nullable
- `skin_tone`: varchar, nullable
- `face_shape`: varchar, nullable
- `body_proportion`: varchar, nullable
- `outfit_top`: varchar, nullable
- `outfit_bottom`: varchar, nullable
- `accessory`: varchar, nullable
- `visual_must_keep`: jsonb, not null, default `[]`
- `negative_rules`: jsonb, not null, default `[]`
- `active_reference_image_id`: uuid, nullable, FK `reference_images.id`
- `status`: varchar, not null, enum `draft` / `active` / `superseded`
- `created_at`: timestamptz, not null

### `prop_profiles`

道具卡是图片稳定性方案中的缺口。PRD 没要求每个道具都建卡，但图片稳定性文档明确要求重复出现、强身份相关或剧情推进道具需要固定。

- `id`: uuid, primary key
- `storybook_id`: uuid, nullable, FK `storybooks.id`
- `child_id`: uuid, nullable, FK `children.id`
- `name`: varchar, not null
- `shape`: varchar, nullable
- `primary_color`: varchar, nullable
- `secondary_color`: varchar, nullable
- `material_style`: varchar, nullable
- `size_description`: varchar, nullable
- `visual_must_keep`: jsonb, not null, default `[]`
- `negative_rules`: jsonb, not null, default `[]`
- `active_reference_image_id`: uuid, nullable, FK `reference_images.id`
- `status`: varchar, not null, enum `draft` / `active` / `archived`
- `created_at`: timestamptz, not null
- `updated_at`: timestamptz, not null

### `reference_images`

角色、家长角色、道具都可以拥有参考图，所以不要只绑定 `character_profile_id`。

- `id`: uuid, primary key
- `subject_type`: varchar, not null, enum `child_character` / `parent_character` / `prop`
- `character_profile_id`: uuid, nullable, FK `character_profiles.id`
- `parent_character_profile_id`: uuid, nullable, FK `parent_character_profiles.id`
- `prop_profile_id`: uuid, nullable, FK `prop_profiles.id`
- `image_asset_id`: uuid, not null, FK `image_assets.id`
- `source_task_id`: uuid, nullable, FK `image_generation_tasks.id`
- `style_id`: varchar, not null
- `review_status`: varchar, not null, enum `pending` / `approved` / `rejected`
- `is_active`: boolean, not null, default `false`
- `created_at`: timestamptz, not null

Rule:

- Exactly one subject FK must be non-null based on `subject_type`.
- Only one active reference image per `(subject_type, subject_id, style_id)` should be allowed.

### `storybook_roles`

`role_manifest_json` is useful for response payloads, but role replacement should also have queryable rows.

- `id`: uuid, primary key
- `storybook_id`: uuid, not null, FK `storybooks.id`
- `role_key`: varchar, not null
- `role_type`: varchar, not null, enum `child` / `parent` / `teacher` / `default_character` / `prop`
- `display_name`: varchar, not null
- `child_id`: uuid, nullable, FK `children.id`
- `character_profile_id`: uuid, nullable, FK `character_profiles.id`
- `parent_character_profile_id`: uuid, nullable, FK `parent_character_profiles.id`
- `prop_profile_id`: uuid, nullable, FK `prop_profiles.id`
- `replacement_source_role_id`: uuid, nullable, FK `storybook_roles.id`
- `created_at`: timestamptz, not null
- `updated_at`: timestamptz, not null

Rule:

- `(storybook_id, role_key)` unique.
- This table is source of truth for role replacement; `role_manifest_json` can be cached/serialized from it.

### `storybook_page_roles`

- `id`: uuid, primary key
- `storybook_page_id`: uuid, not null, FK `storybook_pages.id`
- `storybook_role_id`: uuid, not null, FK `storybook_roles.id`
- `importance`: varchar, not null, enum `primary` / `medium` / `low`
- `placement_hint`: varchar, nullable
- `created_at`: timestamptz, not null

Rule:

- `(storybook_page_id, storybook_role_id)` unique.
- Supports editor side panel and page-specific image prompt assembly.

### `image_assets`

- `id`: uuid, primary key
- `asset_type`: varchar, not null, enum `child_photo` / `reference_image` / `storybook_page_image` / `export_pdf` / `qrcode`
- `storage_url`: varchar, not null
- `storage_key`: varchar, nullable
- `mime_type`: varchar, nullable
- `width`: integer, nullable
- `height`: integer, nullable
- `file_size`: bigint, nullable
- `checksum`: varchar, nullable
- `review_result`: varchar, nullable, enum `pending` / `approved` / `rejected`
- `metadata_json`: jsonb, not null, default `{}`
- `created_at`: timestamptz, not null

Rule:

- `checksum` should be indexed if duplicate detection matters.
- `metadata_json` may store provider returned IDs, but not the only copy of business-critical task state.

### `image_generation_tasks`

This is the core tracking table. It stores task lifecycle and immutable input snapshots.

- `id`: uuid, primary key
- `idempotency_key`: varchar, nullable
- `task_type`: varchar, not null, enum `reference_image_generation` / `prop_reference_generation` / `page_image_generation` / `storybook_image_generation`
- `parent_task_id`: uuid, nullable, FK `image_generation_tasks.id`
- `retry_of_task_id`: uuid, nullable, FK `image_generation_tasks.id`
- `school_id`: uuid, nullable, FK `schools.id`
- `teacher_id`: uuid, nullable, FK `teachers.id`
- `storybook_id`: uuid, nullable, FK `storybooks.id`
- `storybook_page_id`: uuid, nullable, FK `storybook_pages.id`
- `character_profile_id`: uuid, nullable, FK `character_profiles.id`
- `character_profile_version`: integer, nullable
- `reference_image_id`: uuid, nullable, FK `reference_images.id`
- `style_id`: varchar, not null
- `prompt_template_version`: varchar, not null
- `scene_spec_json`: jsonb, nullable
- `input_snapshot_json`: jsonb, not null, default `{}`
- `raw_prompt_text`: text, nullable
- `provider_name`: varchar, nullable
- `model_name`: varchar, nullable
- `provider_request_id`: varchar, nullable
- `status`: varchar, not null, enum `queued` / `running` / `succeeded` / `failed` / `needs_review` / `cancelled`
- `failure_reason`: varchar, nullable, enum `character_inconsistent` / `scene_mismatch` / `composition_mismatch` / `quality_artifact` / `unsafe_content` / `provider_error` / `timeout` / `unknown`
- `retry_count`: integer, not null, default `0`
- `max_retries`: integer, not null, default `2`
- `queued_at`: timestamptz, not null
- `started_at`: timestamptz, nullable
- `completed_at`: timestamptz, nullable
- `created_at`: timestamptz, not null
- `updated_at`: timestamptz, not null

Rules:

- `storybook_image_generation` is a parent task; page tasks reference it through `parent_task_id`.
- `page_image_generation` must have `storybook_page_id`.
- Reference tasks must have exactly one target subject in `input_snapshot_json` or a subject FK via `character_profile_id` / prop reference flow.
- `character_profile_id` is nullable because plain storybooks may use default roles or no child-bound protagonist.
- Page image tasks require `storybook_pages.scene_spec_status = ready`, not merely a storybook id.
- `input_snapshot_json` must include character profile, parent character profiles, prop profiles, style spec and page scene spec used at dispatch time.
- `idempotency_key` prevents duplicate task creation from repeated frontend clicks.

### `image_generation_outputs`

Do not store only one output on the task. A provider may return multiple candidates; review can reject one and approve another.

- `id`: uuid, primary key
- `task_id`: uuid, not null, FK `image_generation_tasks.id`
- `image_asset_id`: uuid, not null, FK `image_assets.id`
- `candidate_index`: integer, not null, default `0`
- `is_selected`: boolean, not null, default `false`
- `review_status`: varchar, not null, enum `pending` / `approved` / `rejected`
- `quality_notes`: text, nullable
- `created_at`: timestamptz, not null

Rules:

- `(task_id, candidate_index)` unique.
- Only one selected output per task should be allowed.
- When a page output is selected, update `storybook_pages.current_image_asset_id` and `current_image_task_id`.

### `image_review_events`

- `id`: uuid, primary key
- `task_id`: uuid, not null, FK `image_generation_tasks.id`
- `output_id`: uuid, nullable, FK `image_generation_outputs.id`
- `reviewer_teacher_id`: uuid, nullable, FK `teachers.id`
- `review_action`: varchar, not null, enum `approve` / `reject` / `request_retry` / `select_candidate`
- `reason_code`: varchar, nullable
- `notes`: text, nullable
- `created_at`: timestamptz, not null

This gives failure analysis data without overloading task status.

### `generation_cost_logs`

- `id`: uuid, primary key
- `task_id`: uuid, not null, FK `image_generation_tasks.id`
- `school_id`: uuid, nullable, FK `schools.id`
- `teacher_id`: uuid, nullable, FK `teachers.id`
- `storybook_id`: uuid, nullable, FK `storybooks.id`
- `storybook_page_id`: uuid, nullable, FK `storybook_pages.id`
- `provider_name`: varchar, not null
- `model_name`: varchar, not null
- `input_units`: numeric, nullable
- `output_units`: numeric, nullable
- `input_cost`: numeric(12,4), not null
- `output_cost`: numeric(12,4), not null
- `total_cost`: numeric(12,4), not null
- `currency`: varchar, not null
- `billed_units_json`: jsonb, not null, default `{}`
- `created_at`: timestamptz, not null

Rule:

- Cost records are append-only.
- Reporting should aggregate by `school_id`, `teacher_id`, `storybook_id`, model and time.

### `storybook_exports`

- `id`: uuid, primary key
- `storybook_id`: uuid, not null, FK `storybooks.id`
- `export_type`: varchar, not null, enum `pdf` / `flipbook` / `print_layout`
- `status`: varchar, not null, enum `queued` / `running` / `succeeded` / `failed`
- `asset_id`: uuid, nullable, FK `image_assets.id`
- `failure_reason`: text, nullable
- `created_by`: uuid, not null, FK `teachers.id`
- `created_at`: timestamptz, not null
- `completed_at`: timestamptz, nullable

### `storybook_share_links`

- `id`: uuid, primary key
- `storybook_id`: uuid, not null, FK `storybooks.id`
- `share_scope`: varchar, not null, enum `family` / `school` / `platform_review` / `platform_public`
- `token_hash`: varchar, not null
- `qrcode_asset_id`: uuid, nullable, FK `image_assets.id`
- `anonymize_child_name`: boolean, not null, default `true`
- `anonymize_parent_info`: boolean, not null, default `true`
- `status`: varchar, not null, enum `active` / `disabled` / `expired`
- `created_by`: uuid, not null, FK `teachers.id`
- `created_at`: timestamptz, not null
- `expires_at`: timestamptz, nullable

## Relationships

- `school` owns teachers, classrooms, children, storybooks and image tasks.
- `child` has many character profile versions and optional photos.
- `story_template` provides reusable skeletons.
- `case_storybook` exposes finished cases to teachers.
- `storybook` has many pages, roles, exports, share links and image generation tasks.
- `storybook_page` has many image tasks over time but one current selected image.
- `character_profile`, `parent_character_profile` and `prop_profile` can each have reference images.
- `image_generation_task` can be a parent batch task or a single page/reference task.
- `image_generation_output` maps tasks to generated image assets.
- `generation_cost_log` belongs to a task and denormalizes school/teacher/storybook/page for reporting.

## Constraints and validation

- `children.name` required.
- `children.profile_completion_status = usable` requires at least one of `hair`, `usual_outfit`, `favorite_color`, `interest_tags` or a primary photo.
- `storybooks.content_type = custom_storybook` requires `child_id`.
- `storybooks.content_type = plain_storybook` may have `child_id = null`.
- `storybook_pages(storybook_id, page_number)` unique.
- `storybook_roles(storybook_id, role_key)` unique.
- `storybook_page_roles(storybook_page_id, storybook_role_id)` unique.
- `character_profiles(child_id, version)` unique.
- `parent_character_profiles(parent_id, child_id, version)` unique when `child_id` is present.
- `image_generation_tasks.idempotency_key` unique when present.
- `image_generation_outputs(task_id, candidate_index)` unique.
- `image_generation_tasks.status` must follow queued -> running -> succeeded/needs_review/failed/cancelled.
- `storybook_pages.is_locked = true` must prevent automatic text/image overwrite, but manual explicit edits may still be allowed.
- `reference_images` must have exactly one subject FK based on `subject_type`.

## Indexes and query patterns

### Core indexes

- `teachers(school_id, status)`
- `classrooms(school_id, status)`
- `children(primary_teacher_id, updated_at desc)`
- `children(classroom_id, updated_at desc)`
- `children(school_id, profile_completion_status)`
- `story_templates(content_type, theme, status)`
- `case_storybooks(theme, target_age_group, status, sort_order)`
- `storybooks(teacher_id, updated_at desc)`
- `storybooks(child_id, created_at desc)`
- `storybooks(school_id, status, updated_at desc)`
- `storybook_pages(storybook_id, page_number)` unique
- `storybook_roles(storybook_id, role_key)` unique
- `storybook_page_roles(storybook_page_id)`
- `character_profiles(child_id, version desc)`
- `reference_images(subject_type, style_id, is_active)`

### Image generation indexes

- `image_generation_tasks(status, queued_at)`
- `image_generation_tasks(parent_task_id)`
- `image_generation_tasks(storybook_id, status)`
- `image_generation_tasks(storybook_page_id, status, created_at desc)`
- `image_generation_tasks(character_profile_id, created_at desc)`
- `image_generation_tasks(provider_name, model_name, created_at desc)`
- `image_generation_tasks(idempotency_key)` unique where not null
- `image_generation_outputs(task_id, candidate_index)` unique
- `image_generation_outputs(image_asset_id)`
- `image_review_events(task_id, created_at desc)`
- `generation_cost_logs(school_id, created_at desc)`
- `generation_cost_logs(teacher_id, created_at desc)`
- `generation_cost_logs(storybook_id, created_at desc)`
- `generation_cost_logs(provider_name, model_name, created_at desc)`

### Primary query patterns

- 工作台：按 teacher 查询最近读本、故事生成中读本、待插图读本、待导出读本、待补档孩子、运行中图片任务。
- 案例库：按 theme、age_group、status 查询成品案例和封面图。
- 读本编辑页：一次取 storybook、pages、roles、page_roles、scene spec 状态、current image assets、running tasks。
- 图片任务轮询：按 task id 返回状态、输出候选、成本汇总和失败原因。
- 单页重绘：创建 page task，成功后写 output，老师确认后更新 page current image。
- 故事生成：先创建 storybook 和 pages；故事完成后再进入图片阶段。
- 整本图片生成：创建 parent image task，再为每页创建 child image task，父任务状态由子任务聚合。
- 成本后台：按 school、teacher、model、date 聚合 generation_cost_logs。

## Lifecycle rules

- 儿童档案可以从家长提交开始，老师补齐教学标签后变为 usable。
- 角色卡显著变化必须创建新版本，不覆盖历史版本。
- 参考图可多次生成，只有审核通过的一张成为 active。
- 普通绘本可作为母本，派生定制绘本时先复制/改写故事页并新建 storybook，再按需触发图片重绘。
- 角色替换优先改写 `storybook_roles` 和页面角色映射，不默认触发整本重绘。
- 页面编辑即时写 `storybook_pages`；锁定页不参与自动重写和自动重绘。
- 单页重绘必须在故事页存在且 `scene_spec_status = ready` 后创建新的 `image_generation_tasks`，不覆盖旧任务。
- 图片输出先进入 `image_generation_outputs`，被选择后才成为页面 current image。
- 任务失败后 retry 创建新任务，并通过 `retry_of_task_id` 关联原任务。
- 成本记录 append-only，不随任务删除。
- 删除组织、儿童、读本时优先软删除或归档，不硬删历史生成资产。

## Migration notes

### Current migration gap

当前 `server/migration/src/m20250603_000001_create_storybook_schema.rs` 已覆盖：

- `teachers`
- `children`
- `story_templates`
- `image_assets`
- `character_profiles`
- `storybooks`
- `storybook_pages`
- `image_generation_tasks`
- `generation_cost_logs`

但和本设计相比仍缺：

- `schools`、`classrooms`、`parents`
- `child_photos`
- `parent_character_profiles`
- `prop_profiles`
- `reference_images`
- `case_storybooks`
- `storybook_roles`
- `storybook_page_roles`
- `image_generation_outputs`
- `image_review_events`
- `storybook_exports`
- `storybook_share_links`

已有表建议补齐：

- `children`: add `school_id`, `primary_parent_id`, `hair`, `skin_tone`, `teacher_observation_tags`, `teaching_focus`, `profile_completion_status`
- `storybooks`: add `school_id`, `case_storybook_id`, `generation_config_json`, `exported_at`
- `storybooks`: split story and illustration states with `story_status` and `illustration_status`
- `storybook_pages`: rename or alias `image_asset_id` to `current_image_asset_id`; add `current_image_task_id`, `illustration_status`, `prompt_text`, `scene_spec_status`
- `image_generation_tasks`: add `idempotency_key`, `retry_of_task_id`, `school_id`, `teacher_id`, `storybook_page_id` naming consistency, `input_snapshot_json`, `provider_request_id`, `queued_at`, `started_at`, `max_retries`
- `generation_cost_logs`: add `school_id`, `teacher_id`, `storybook_id`, `storybook_page_id`, `input_units`, `output_units`

### Suggested migration sequence

1. Add organization and parent tables: `schools`, `classrooms`, `parents`.
2. Add profile completion fields to `children`.
3. Add role and consistency tables: `parent_character_profiles`, `prop_profiles`, `reference_images`, `storybook_roles`, `storybook_page_roles`.
4. Add image tracking tables: `image_generation_outputs`, `image_review_events`.
5. Add export/share tables.
6. Backfill existing `storybooks.role_manifest_json` into `storybook_roles`.
7. Backfill current page image references into `image_generation_outputs` only where task history exists; otherwise keep as current image only.

## Risks or alternative models

- If the project remains very small, `storybook_roles` and `storybook_page_roles` could initially be stored only in JSON. This reduces migrations but makes role replacement, editor sidebars and prompt assembly harder to query and validate.
- If providers return only one image candidate, `image_generation_outputs` still pays off because review and retry history stay clean.
- If cost reporting is not needed in MVP, denormalized `school_id` and `teacher_id` could be omitted from `generation_cost_logs`, but then every report requires joins through tasks and storybooks.
- If sharing is deferred, `storybook_share_links` can wait, but `share_scope` on storybooks should remain because privacy state affects UI and exports.

## 架构 Review

### Primary workflow map

1. 老师选择案例和孩子生成故事
   - UI: 生成配置页提交孩子、案例、画风、年龄段和教学目标。
   - API: `POST /api/storybooks/generate`
   - DB write: `storybooks`, `storybook_pages`, `storybook_roles`, `storybook_page_roles`
   - Validation: custom storybook requires child and usable profile; plain storybook may omit child.
   - Result: `storybooks.story_status = story_ready`; no page image task is required for story text completion.

2. 系统准备图片稳定性输入
   - UI: 儿童档案或生成流程中显示参考图任务。
   - API: `POST /api/images/reference`
   - DB write: `image_generation_tasks`, `image_generation_outputs`, `reference_images`
   - Validation: character profile must have enough visual anchors.
   - Timing: happens after story pages exist, except pre-existing child角色卡/reference image can be reused.

3. 系统生成整本插图
   - UI: 读本编辑页显示每页状态。
   - API: `POST /api/images/storybooks`
   - DB write: one parent `image_generation_tasks`, many page child tasks
   - Validation: `story_status = story_ready`; locked pages are skipped unless explicit override; page `scene_spec_status = ready`.

4. 老师单页编辑和重绘
   - UI: 页面缩略图、当前页预览、文字编辑区、插图重绘区。
   - API: `PATCH /api/storybooks/:id/pages/:pageNumber`, `POST /api/images/pages`
   - DB write: `storybook_pages`, `image_generation_tasks`, `image_generation_outputs`
   - Validation: page belongs to teacher/school; scene spec must contain location/action/emotion/composition for image generation.

5. 老师替换角色派生定制版本
   - UI: 角色替换面板。
   - API: `POST /api/storybooks/:id/replace-roles`
   - DB write: new `storybooks`, copied `storybook_pages`, new `storybook_roles`, inherited source links
   - Validation: replacement child must have usable character profile; image reuse first, page tasks only where roles actually appear.

6. 老师导出与分享
   - UI: 导出 PDF、电子版、二维码、分享设置。
   - API: export/share endpoints
   - DB write: `storybook_exports`, `storybook_share_links`, optional `image_assets`
   - Validation: sharing requires privacy settings and child/parent anonymization flags.

### Database support check

- Supports PRD editor requirements: page thumbnails, page preview, text edit, lock, add/delete pages, single-page rewrite and redraw.
- Supports corrected order: story and pages can be created and edited before any image task exists.
- Supports image stability: character profiles, parent profiles, prop profiles, reference images and immutable task input snapshots.
- Supports retry and cost analysis: task status, retry linkage, failure reasons, outputs, review events and cost logs.
- Supports role replacement: queryable storybook roles and page-role mapping.
- Supports ordinary-to-custom derivation: `source_storybook_id` and `derivation_type`.

### API support check

Existing API contract mostly fits, but should be adjusted:

- `POST /api/storybooks/generate` should return story generation status and pages; `image_batch_task_id` should be optional or omitted until image generation is explicitly started.
- Add or document `POST /api/storybooks/:id/images/start` / use `POST /api/images/storybooks` only after story is ready.
- `POST /api/images/pages` should accept an `idempotency_key`.
- `GET /api/image-tasks/:taskId` should return `outputs[]`, not a single `image_asset`, because candidates and review status matter.
- `POST /api/image-tasks/:taskId/retry` should create a new task with `retry_of_task_id`.
- `GET /api/storybooks/:id` should include `roles`, `page_roles`, `current_image_asset`, and active image task per page.
- `POST /api/storybooks/:id/replace-roles` should document whether it copies images, creates new page tasks, or marks pages as needing review.

### Frontend support check

The current dashboard `#studio` page now matches the core editor shape:

- top toolbar: save/share/export
- page thumbnails
- central page preview
- right text editor
- role panel
- image redraw panel
- task status panel

Remaining frontend gaps for later implementation:

- No real API integration yet.
- Add/delete page and lock state are currently toast-only.
- Role replacement panel is represented as an entry point, not a full flow.
- Image task polling and output candidate review UI are not implemented yet.

### Inconsistencies found

1. Old `reference_images` design only supported child character reference images.
   - Fix: generalized `reference_images.subject_type` and subject FKs.

2. Old `storybook_pages.image_asset_id` mixed current image with generation history.
   - Fix: `current_image_asset_id` on page plus historical `image_generation_outputs`.

3. Old image task design lacked immutable input snapshots.
   - Fix: added `input_snapshot_json`, `raw_prompt_text`, `prompt_template_version`.

4. Old model had `role_manifest_json` but no queryable role rows.
   - Fix: added `storybook_roles` and `storybook_page_roles`.

5. Prompt spec included `prop_profiles`, but database did not.
   - Fix: added `prop_profiles` and prop reference support.

6. Cost logs lacked enough denormalized reporting dimensions.
   - Fix: added school, teacher, storybook and page fields.

7. API task response showed one `image_asset`.
   - Fix recommended: return `outputs[]` and selected output.

8. Previous review implied image generation could be part of initial story generation response.
   - Fix: story generation and image generation are separated. Story pages are the source input for the later image stability pipeline.

### Recommended adjustments

- Treat this document as the revised target schema for image generation.
- Update `完整版数据库结构设计.md` or supersede it with this design before coding a new migration.
- Add a follow-up SQL/SeaORM migration plan from current schema to this target.
- Update API contract so `POST /api/storybooks/generate` is story-first and image task ids are created by explicit image endpoints after `story_status = story_ready`.
- Update API contract task response to include multiple outputs and review status.
- Keep JSON fields for prompt inputs, but do not use JSON as the only source of role replacement and task lifecycle truth.

### Frozen phase-1 blueprint

Phase 1 database should include at minimum:

- Existing core: `teachers`, `children`, `story_templates`, `storybooks`, `storybook_pages`, `character_profiles`, `image_assets`, `image_generation_tasks`, `generation_cost_logs`
- Add before image implementation: `parents`, `child_photos`, `parent_character_profiles`, `prop_profiles`, `reference_images`, `storybook_roles`, `storybook_page_roles`, `image_generation_outputs`
- Add before sharing/export implementation: `storybook_exports`, `storybook_share_links`
- Add before school-wide reporting: `schools`, `classrooms`, cost-log school/teacher/storybook/page dimensions

Do not implement image generation against only `storybook_pages.image_asset_id`; that would block review, retry, cost tracking and candidate selection. Also do not make image generation a prerequisite for story generation completion.
