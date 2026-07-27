use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::models::{DeriveCustomRequest, Storybook, StorybookType};

pub async fn derive_custom(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    source_storybook_id: Uuid,
    payload: DeriveCustomRequest,
) -> Result<Storybook, DbErr> {
    let source =
        crate::repositories::storybook_queries::find(db, workspace_id, source_storybook_id).await?;
    if source.storybook_type != StorybookType::Plain {
        return Err(DbErr::Custom("只有普通绘本可以派生定制绘本".to_string()));
    }
    let child = child_profile_for_custom(db, workspace_id, payload.child_id).await?;
    let plan_strategy = customization_strategy(payload.customization_plan.as_ref());
    let customization =
        build_custom_storybook_customization(&source, &child, &payload.intensity, plan_strategy);
    let new_id = Uuid::new_v4();
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        insert into storybooks
          (id, workspace_id, storybook_type, status, visibility, source, source_storybook_id, target_child_id, title, age_group, use_scene, teaching_goal, cover_tone, creator_id, created_at, updated_at)
        values ($1, $2, 'custom', 'editing', 'private', $3, $4, $5, $6, $7, $8, $9, $10, '00000000-0000-0000-0000-000000000001', now(), now())
        "#,
        [
            new_id.into(),
            workspace_id.into(),
            customization.source.into(),
            source_storybook_id.into(),
            payload.child_id.into(),
            customization.title.into(),
            customization.age_group.into(),
            customization.use_scene.into(),
            customization.teaching_goal.into(),
            customization.cover_tone.into(),
        ],
    ))
    .await?;
    crate::repositories::storybook_factory::clone_pages_and_roles(db, source_storybook_id, new_id)
        .await?;
    apply_child_customization(db, new_id, &child, &payload.intensity).await?;
    crate::repositories::storybook_queries::find(db, workspace_id, new_id).await
}

fn build_custom_storybook_customization(
    source: &Storybook,
    child: &CustomChildProfile,
    intensity: &str,
    plan_strategy: Option<String>,
) -> CustomStorybookCustomization {
    let mut teaching_goal = source.teaching_goal.clone();
    if let Some(strategy) = plan_strategy {
        teaching_goal.push_str(&format!("；定制方案：{strategy}"));
    }

    CustomStorybookCustomization {
        source: format!("derived:{intensity}"),
        title: format!("{}的定制故事", child.nickname),
        age_group: source.age_group.clone(),
        use_scene: source.use_scene.clone(),
        teaching_goal,
        cover_tone: source.cover_tone.clone(),
    }
}

fn customization_strategy(plan: Option<&JsonValue>) -> Option<String> {
    plan.and_then(|value| {
        value
            .get("customization")
            .and_then(|customization| customization.get("strategy"))
            .or_else(|| value.get("strategy"))
            .and_then(|strategy| strategy.as_str())
            .filter(|strategy| !strategy.trim().is_empty())
            .map(|strategy| strategy.trim().to_string())
    })
}

struct CustomStorybookCustomization {
    source: String,
    title: String,
    age_group: String,
    use_scene: String,
    teaching_goal: String,
    cover_tone: String,
}

struct CustomChildProfile {
    nickname: String,
    interests: Vec<String>,
    traits: Vec<String>,
    focus: String,
}

async fn child_profile_for_custom(
    db: &DatabaseConnection,
    workspace_id: Uuid,
    child_id: Uuid,
) -> Result<CustomChildProfile, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
        select nickname, interests, traits, coalesce(focus, '') as focus
        from children
        where workspace_id = $1 and id = $2 and status = 'active'
        "#,
            [workspace_id.into(), child_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("child".to_string()))?;
    Ok(CustomChildProfile {
        nickname: row.try_get("", "nickname")?,
        interests: json_string_list(row.try_get("", "interests")?),
        traits: json_string_list(row.try_get("", "traits")?),
        focus: row.try_get("", "focus")?,
    })
}

async fn apply_child_customization(
    db: &DatabaseConnection,
    storybook_id: Uuid,
    child: &CustomChildProfile,
    intensity: &str,
) -> Result<(), DbErr> {
    let interest_text = child.interests.join("、");
    let trait_text = child.traits.join("、");
    let first_interest = child
        .interests
        .first()
        .cloned()
        .unwrap_or_else(|| "喜欢的活动".to_string());
    let focus = if child.focus.trim().is_empty() {
        "当前教学目标"
    } else {
        child.focus.as_str()
    };

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_roles
        set name = $2,
            role_type = 'protagonist',
            appearance = $3,
            story_function = $4,
            needs_consistency = true
        where id = (
            select id
            from storybook_roles
            where storybook_id = $1 and role_type in ('protagonist', 'peer', 'supporting')
            order by case role_type when 'protagonist' then 0 when 'peer' then 1 else 2 end, name
            limit 1
        )
        "#,
        [
            storybook_id.into(),
            child.nickname.clone().into(),
            format!(
                "{}，带有孩子熟悉的兴趣元素：{}",
                if trait_text.is_empty() {
                    "幼儿园孩子"
                } else {
                    &trait_text
                },
                if interest_text.is_empty() {
                    "日常游戏"
                } else {
                    &interest_text
                }
            )
            .into(),
            format!("以{}的视角练习{}", child.nickname, focus).into(),
        ],
    ))
    .await?;

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybook_pages
        set title = case when page_number = 1 then $2 else title end,
            body = body || $3,
            illustration_prompt = illustration_prompt || $4,
            status = 'needs_regeneration'
        where storybook_id = $1
        "#,
        [
            storybook_id.into(),
            format!("{}来到故事里", child.nickname).into(),
            format!(
                "\n\n定制改写：这一版会称呼{}，结合{}，重点练习{}。",
                child.nickname,
                if interest_text.is_empty() {
                    "孩子熟悉的生活经验"
                } else {
                    &interest_text
                },
                focus
            )
            .into(),
            format!(
                "；定制版加入{}熟悉的{}元素，保持角色跨页一致",
                child.nickname, first_interest
            )
            .into(),
        ],
    ))
    .await?;

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        update storybooks
        set teaching_goal = teaching_goal || $2,
            cover_tone = $3,
            updated_at = now()
        where id = $1
        "#,
        [
            storybook_id.into(),
            format!("；定制关注：{}（{}）", focus, intensity).into(),
            format!(
                "定制给{}，融合{}",
                child.nickname,
                if interest_text.is_empty() {
                    "孩子日常经验"
                } else {
                    &interest_text
                }
            )
            .into(),
        ],
    ))
    .await?;

    Ok(())
}

fn json_string_list(value: JsonValue) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str())
                .filter(|item| !item.trim().is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{StorybookPage, StorybookRole, StorybookStatus, Visibility};

    #[test]
    fn build_custom_storybook_customization_keeps_source_story_context() {
        let source = test_storybook();
        let child = CustomChildProfile {
            nickname: "乐乐".to_string(),
            interests: vec!["积木车".to_string(), "唱歌".to_string()],
            traits: vec!["热情".to_string()],
            focus: "轮流和表达需求".to_string(),
        };

        let customization = build_custom_storybook_customization(&source, &child, "balanced", None);

        assert_eq!(customization.source, "derived:balanced");
        assert_eq!(customization.title, "乐乐的定制故事");
        assert_eq!(customization.age_group, source.age_group);
        assert_eq!(customization.use_scene, source.use_scene);
        assert_eq!(customization.teaching_goal, source.teaching_goal);
        assert_eq!(customization.cover_tone, source.cover_tone);
    }

    #[test]
    fn customization_strategy_reads_confirmed_generation_plan() {
        let plan = serde_json::json!({
            "customization": {
                "strategy": "保留母本主线，替换孩子称呼和兴趣道具。"
            }
        });

        assert_eq!(
            customization_strategy(Some(&plan)).as_deref(),
            Some("保留母本主线，替换孩子称呼和兴趣道具。")
        );
    }

    fn test_storybook() -> Storybook {
        Storybook {
            id: Uuid::new_v4(),
            workspace_id: Uuid::new_v4(),
            title: "一起玩小汽车".to_string(),
            storybook_type: StorybookType::Plain,
            status: StorybookStatus::Editing,
            visibility: Visibility::Private,
            source: "blank".to_string(),
            source_title: None,
            target_child_id: None,
            creator_name: "林老师".to_string(),
            updated_at: "今天 09:00".to_string(),
            age_group: "4-5 岁".to_string(),
            use_scene: "规则引导".to_string(),
            teaching_goal: "学习轮流与分享".to_string(),
            cover_tone: "温暖、清楚".to_string(),
            teacher_review_status: "pending".to_string(),
            teacher_reviewed_by: None,
            teacher_reviewed_at: None,
            pages: vec![StorybookPage {
                id: Uuid::new_v4(),
                page_number: 1,
                title: "第一页".to_string(),
                body: "内容".to_string(),
                illustration_prompt: "提示".to_string(),
                status: "ready".to_string(),
            }],
            roles: vec![StorybookRole {
                id: Uuid::new_v4(),
                name: "林老师".to_string(),
                role_type: "teacher".to_string(),
                appearance: "温和、稳定".to_string(),
                story_function: "引导孩子轮流等待".to_string(),
                needs_consistency: true,
                reference_image_url: None,
                reference_image_prompt: None,
                reference_status: "not_started".to_string(),
            }],
            quality: Default::default(),
        }
    }
}
