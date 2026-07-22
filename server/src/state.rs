use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use uuid::Uuid;

use crate::models::*;

pub type SharedState = Arc<RwLock<AppState>>;
pub const DEV_TOKEN: &str = "dev-token";

#[derive(Debug)]
pub struct AppState {
    pub current_user: User,
    pub token: String,
    pub workspaces: Vec<Workspace>,
    pub members: Vec<WorkspaceMember>,
    pub classrooms: Vec<Classroom>,
    pub children: Vec<ChildProfile>,
    pub storybooks: Vec<Storybook>,
    pub templates: Vec<MarketplaceTemplate>,
    pub submissions: Vec<MarketplaceSubmission>,
    pub share_links: HashMap<String, ShareLink>,
}

pub fn seed_state() -> SharedState {
    let user_id = id("00000000-0000-0000-0000-000000000001");
    let personal = id("10000000-0000-0000-0000-000000000001");
    let school_admin = id("20000000-0000-0000-0000-000000000001");
    let school_teacher = id("20000000-0000-0000-0000-000000000002");
    let child_1 = id("30000000-0000-0000-0000-000000000001");
    let child_2 = id("30000000-0000-0000-0000-000000000002");
    let child_3 = id("30000000-0000-0000-0000-000000000003");
    let story_1 = id("40000000-0000-0000-0000-000000000001");
    let story_2 = id("40000000-0000-0000-0000-000000000002");
    let story_3 = id("40000000-0000-0000-0000-000000000003");
    let story_4 = id("40000000-0000-0000-0000-000000000004");

    Arc::new(RwLock::new(AppState {
        current_user: User {
            id: user_id,
            display_name: "林老师".to_string(),
            email: "lin@example.com".to_string(),
        },
        token: DEV_TOKEN.to_string(),
        workspaces: vec![
            Workspace {
                id: personal,
                name: "林老师的个人空间".to_string(),
                workspace_type: WorkspaceType::Personal,
                role: WorkspaceRole::PersonalOwner,
                description: "个人绘本、我的孩子资料和私人导出记录".to_string(),
            },
            Workspace {
                id: school_admin,
                name: "星星幼儿园".to_string(),
                workspace_type: WorkspaceType::School,
                role: WorkspaceRole::SchoolAdmin,
                description: "园所绘本、班级儿童、老师协作和市场投稿".to_string(),
            },
            Workspace {
                id: school_teacher,
                name: "彩虹幼儿园".to_string(),
                workspace_type: WorkspaceType::School,
                role: WorkspaceRole::SchoolTeacher,
                description: "授权班级：中一班、小二班".to_string(),
            },
        ],
        members: vec![
            WorkspaceMember {
                id: Uuid::new_v4(),
                workspace_id: school_admin,
                name: "王老师".to_string(),
                email: "wang@example.com".to_string(),
                role: WorkspaceRole::SchoolTeacher,
                status: "active".to_string(),
                classes: vec!["小一班".to_string()],
                invitation_token: None,
                invitation_url: None,
            },
            WorkspaceMember {
                id: Uuid::new_v4(),
                workspace_id: school_admin,
                name: "园长李老师".to_string(),
                email: "admin@example.com".to_string(),
                role: WorkspaceRole::SchoolAdmin,
                status: "active".to_string(),
                classes: vec!["全部".to_string()],
                invitation_token: None,
                invitation_url: None,
            },
        ],
        classrooms: vec![
            Classroom {
                id: Uuid::new_v4(),
                workspace_id: school_admin,
                name: "小一班".to_string(),
                age_group: "3-4 岁".to_string(),
                teachers: 2,
                children: 18,
                status: "active".to_string(),
            },
            Classroom {
                id: Uuid::new_v4(),
                workspace_id: school_teacher,
                name: "中一班".to_string(),
                age_group: "4-5 岁".to_string(),
                teachers: 1,
                children: 21,
                status: "active".to_string(),
            },
        ],
        children: vec![
            ChildProfile {
                id: child_1,
                workspace_id: personal,
                nickname: "乐乐".to_string(),
                age_group: "4-5 岁".to_string(),
                classroom: None,
                interests: tags(["积木车", "蓝色", "小火车"]),
                traits: tags(["热情", "需要练习等待"]),
                focus: "轮流和表达需求".to_string(),
                completeness: 92,
                status: "active".to_string(),
                updated_at: "今天 08:30".to_string(),
            },
            ChildProfile {
                id: child_2,
                workspace_id: school_admin,
                nickname: "小雨".to_string(),
                age_group: "3-4 岁".to_string(),
                classroom: Some("小一班".to_string()),
                interests: tags(["贴纸", "小兔", "唱歌"]),
                traits: tags(["慢热", "喜欢被鼓励"]),
                focus: "入园适应和午睡".to_string(),
                completeness: 76,
                status: "active".to_string(),
                updated_at: "昨天 17:40".to_string(),
            },
            ChildProfile {
                id: child_3,
                workspace_id: school_teacher,
                nickname: "安安".to_string(),
                age_group: "4-5 岁".to_string(),
                classroom: Some("中一班".to_string()),
                interests: tags(["恐龙", "搭桥", "绿色"]),
                traits: tags(["好奇", "表达直接"]),
                focus: "排队等待".to_string(),
                completeness: 84,
                status: "active".to_string(),
                updated_at: "周一 12:18".to_string(),
            },
        ],
        storybooks: vec![
            storybook(
                story_1,
                personal,
                "一起玩小汽车",
                StorybookType::Plain,
                StorybookStatus::Exportable,
                Visibility::Private,
                None,
                None,
                "学习轮流与分享",
                "规则引导",
            ),
            storybook(
                story_2,
                personal,
                "乐乐学会一起玩",
                StorybookType::Custom,
                StorybookStatus::Editing,
                Visibility::Private,
                Some("一起玩小汽车"),
                Some(child_1),
                "把轮流等待迁移到家庭场景",
                "家庭共读",
            ),
            storybook(
                story_3,
                school_admin,
                "午睡小小约定",
                StorybookType::Plain,
                StorybookStatus::Submitted,
                Visibility::MarketSubmission,
                Some("安静午睡的一天"),
                None,
                "建立睡前整理和安静入睡流程",
                "午睡习惯",
            ),
            storybook(
                story_4,
                school_teacher,
                "排队像小火车",
                StorybookType::Plain,
                StorybookStatus::Exportable,
                Visibility::Workspace,
                None,
                None,
                "理解排队和等待",
                "规则引导",
            ),
        ],
        templates: vec![
            template(
                "50000000-0000-0000-0000-000000000001",
                "一起玩小汽车",
                "平台精选",
                "platform",
                "围绕分享、轮流和表达感受的 6 页生活化绘本。",
                "规则引导",
            ),
            template(
                "50000000-0000-0000-0000-000000000002",
                "安静午睡的一天",
                "园所投稿",
                "school_submission",
                "帮助小班孩子理解午睡前准备、安静入睡和醒后整理。",
                "午睡习惯",
            ),
        ],
        submissions: vec![MarketplaceSubmission {
            id: id("60000000-0000-0000-0000-000000000001"),
            workspace_id: school_admin,
            title: "午睡小小约定".to_string(),
            source_storybook_title: "午睡小小约定".to_string(),
            submitted_by: "王老师".to_string(),
            status: "submitted".to_string(),
            privacy_confirmed: true,
            updated_at: "今天 11:20".to_string(),
        }],
        share_links: HashMap::new(),
    }))
}

fn id(value: &str) -> Uuid {
    Uuid::parse_str(value).expect("valid seed uuid")
}

fn tags<const N: usize>(values: [&str; N]) -> Vec<String> {
    values.into_iter().map(ToOwned::to_owned).collect()
}

fn storybook(
    id: Uuid,
    workspace_id: Uuid,
    title: &str,
    storybook_type: StorybookType,
    status: StorybookStatus,
    visibility: Visibility,
    source_title: Option<&str>,
    child_id: Option<Uuid>,
    teaching_goal: &str,
    use_scene: &str,
) -> Storybook {
    Storybook {
        id,
        workspace_id,
        title: title.to_string(),
        storybook_type,
        status,
        visibility,
        source: if source_title.is_some() {
            "derived".to_string()
        } else {
            "blank".to_string()
        },
        source_title: source_title.map(ToOwned::to_owned),
        target_child_id: child_id,
        creator_name: "林老师".to_string(),
        updated_at: "今天 10:24".to_string(),
        age_group: "4-5 岁".to_string(),
        use_scene: use_scene.to_string(),
        teaching_goal: teaching_goal.to_string(),
        cover_tone: "温暖、明亮、生活化".to_string(),
        pages: seed_pages(),
        roles: seed_roles(),
    }
}

fn seed_pages() -> Vec<StorybookPage> {
    vec![
        StorybookPage {
            id: Uuid::new_v4(),
            page_number: 1,
            title: "小汽车来到教室".to_string(),
            body: "米米带着红色小汽车来到幼儿园，心里开心又有一点舍不得。".to_string(),
            illustration_prompt: "明亮教室里，白色小兔抱着红色小汽车，朋友们好奇地看着。"
                .to_string(),
            status: "ready".to_string(),
        },
        StorybookPage {
            id: Uuid::new_v4(),
            page_number: 2,
            title: "朋友也想玩".to_string(),
            body: "小熊乐乐轻轻问：我可以一起玩吗？米米把小汽车抱得更紧了。".to_string(),
            illustration_prompt: "小熊伸出手询问，小兔有点犹豫，背景是温暖的积木区。".to_string(),
            status: "needs_regeneration".to_string(),
        },
    ]
}

fn seed_roles() -> Vec<StorybookRole> {
    vec![
        StorybookRole {
            id: Uuid::new_v4(),
            name: "小兔米米".to_string(),
            role_type: "protagonist".to_string(),
            appearance: "白色小兔，圆脸，长耳朵，黄色背带裤".to_string(),
            story_function: "学习分享和轮流的主角".to_string(),
            needs_consistency: true,
        },
        StorybookRole {
            id: Uuid::new_v4(),
            name: "鹿老师".to_string(),
            role_type: "teacher".to_string(),
            appearance: "戴圆眼镜的长颈鹿老师，温柔引导".to_string(),
            story_function: "引导孩子表达和轮流".to_string(),
            needs_consistency: true,
        },
    ]
}

fn template(
    id_value: &str,
    title: &str,
    label: &str,
    source: &str,
    summary: &str,
    scene: &str,
) -> MarketplaceTemplate {
    MarketplaceTemplate {
        id: id(id_value),
        title: title.to_string(),
        summary: summary.to_string(),
        source_type: source.to_string(),
        source_label: label.to_string(),
        source_storybook_id: None,
        age_group: "4-5 岁".to_string(),
        use_scene: scene.to_string(),
        page_count: 6,
        supports_customization: true,
        tags: tags(["分享", "轮流", "课堂共读"]),
    }
}
