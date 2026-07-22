use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use chrono::Utc;
use hmac::{Hmac, Mac};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    models::{LoginResponse, RegisterRequest, User, Workspace, WorkspaceRole, WorkspaceType},
    state::DEV_TOKEN,
};

pub const DEMO_USER_ID: Uuid = Uuid::from_u128(1);

pub async fn seed_demo_account(db: &DatabaseConnection) -> Result<(), DbErr> {
    execute(
        db,
        r#"
        insert into users (id, display_name, email, password_hash, status, created_at, updated_at)
        values
          ('00000000-0000-0000-0000-000000000001', '林老师', 'lin@example.com', 'demo', 'active', now(), now())
        on conflict (email) do update
          set display_name = excluded.display_name,
              status = excluded.status,
              updated_at = now();
        "#,
    )
    .await?;

    execute(
        db,
        r#"
        insert into workspaces (id, name, workspace_type, description, status, created_at, updated_at)
        values
          ('10000000-0000-0000-0000-000000000001', '林老师的个人空间', 'personal', '个人绘本、我的孩子资料和私人导出记录', 'active', now(), now()),
          ('20000000-0000-0000-0000-000000000001', '星星幼儿园', 'school', '园所绘本、班级儿童、老师协作和市场投稿', 'active', now(), now()),
          ('20000000-0000-0000-0000-000000000002', '彩虹幼儿园', 'school', '授权班级：中一班、小二班', 'active', now(), now()),
          ('90000000-0000-0000-0000-000000000001', 'Kindleaf 平台运营', 'platform', '市场审核、审计日志和生成能力监控', 'active', now(), now())
        on conflict (id) do update
          set name = excluded.name,
              workspace_type = excluded.workspace_type,
              description = excluded.description,
              status = excluded.status,
              updated_at = now();
        "#,
    )
    .await?;

    execute(
        db,
        r#"
        insert into workspace_members (id, workspace_id, user_id, role, status, classroom_ids, created_at, updated_at)
        values
          ('70000000-0000-0000-0000-000000000001', '10000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000001', 'personal_owner', 'active', '[]'::jsonb, now(), now()),
          ('70000000-0000-0000-0000-000000000004', '20000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000001', 'school_admin', 'active', '["全部"]'::jsonb, now(), now()),
          ('70000000-0000-0000-0000-000000000002', '20000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000002', 'school_teacher', 'active', '["小一班"]'::jsonb, now(), now()),
          ('70000000-0000-0000-0000-000000000003', '20000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000001', 'school_teacher', 'active', '["中一班", "小二班"]'::jsonb, now(), now()),
          ('70000000-0000-0000-0000-000000000006', '90000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000001', 'platform_operator', 'active', '[]'::jsonb, now(), now())
        on conflict (workspace_id, user_id) do update
          set role = excluded.role,
              status = excluded.status,
              classroom_ids = excluded.classroom_ids,
              updated_at = now();
        "#,
    )
    .await?;

    execute(
        db,
        r#"
        insert into workspace_members (id, workspace_id, user_id, role, status, classroom_ids, created_at, updated_at)
        values
          ('70000000-0000-0000-0000-000000000005', '20000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000001', 'school_admin', 'active', '["全部"]'::jsonb, now(), now())
        on conflict (workspace_id, user_id) do update
          set role = excluded.role,
              status = excluded.status,
              classroom_ids = excluded.classroom_ids,
              updated_at = now();
        "#,
    )
    .await?;

    Ok(())
}

async fn execute(db: &DatabaseConnection, sql: &str) -> Result<(), DbErr> {
    db.execute(Statement::from_string(DbBackend::Postgres, sql.to_string()))
        .await?;
    Ok(())
}

pub async fn login(
    db: &DatabaseConnection,
    identifier: &str,
    password: &str,
) -> Result<LoginResponse, DbErr> {
    let (user, password_hash) = find_user_credentials_by_email(db, identifier).await?;
    if !verify_password(password, password_hash.as_deref()) {
        return Err(DbErr::RecordNotFound("user".to_string()));
    }
    let workspaces = list_workspaces_for_user(db, user.id).await?;
    Ok(LoginResponse {
        token: token_for_user(user.id),
        user,
        workspaces,
    })
}

pub async fn register(
    db: &DatabaseConnection,
    payload: RegisterRequest,
) -> Result<LoginResponse, DbErr> {
    let user_id = Uuid::new_v4();
    let workspace_id = Uuid::new_v4();
    let member_id = Uuid::new_v4();
    let display_name = payload.display_name.trim();
    let email = payload.email.trim().to_lowercase();
    if display_name.is_empty() {
        return Err(DbErr::Custom("请输入显示名称".to_string()));
    }
    if email.is_empty() {
        return Err(DbErr::Custom("请输入邮箱".to_string()));
    }

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        insert into users (id, display_name, email, password_hash, status, created_at, updated_at)
        values ($1, $2, $3, $4, 'active', now(), now())
        "#,
        [
            user_id.into(),
            display_name.into(),
            email.clone().into(),
            hash_password(&payload.password).into(),
        ],
    ))
    .await?;

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        insert into workspaces (id, name, workspace_type, description, status, created_at, updated_at)
        values ($1, $2, 'personal', '个人绘本、孩子资料和私人导出记录', 'active', now(), now())
        "#,
        [
            workspace_id.into(),
            format!("{display_name}的个人空间").into(),
        ],
    ))
    .await?;

    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        insert into workspace_members (id, workspace_id, user_id, role, status, classroom_ids, created_at, updated_at)
        values ($1, $2, $3, 'personal_owner', 'active', '[]'::jsonb, now(), now())
        "#,
        [member_id.into(), workspace_id.into(), user_id.into()],
    ))
    .await?;

    let user = find_user_by_email(db, &email).await?;
    let workspaces = list_workspaces_for_user(db, user.id).await?;
    Ok(LoginResponse {
        token: token_for_user(user.id),
        user,
        workspaces,
    })
}

pub async fn current_session(
    db: &DatabaseConnection,
    user_id: Option<Uuid>,
) -> Result<LoginResponse, DbErr> {
    let user = find_user_by_id(db, user_id.unwrap_or(DEMO_USER_ID)).await?;
    let workspaces = list_workspaces_for_user(db, user.id).await?;
    Ok(LoginResponse {
        token: token_for_user(user.id),
        user,
        workspaces,
    })
}

pub async fn list_current_workspaces(
    db: &DatabaseConnection,
    user_id: Option<Uuid>,
) -> Result<Vec<Workspace>, DbErr> {
    list_workspaces_for_user(db, user_id.unwrap_or(DEMO_USER_ID)).await
}

async fn find_user_by_email(db: &DatabaseConnection, email: &str) -> Result<User, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id, display_name, email
            from users
            where email = $1 and status = 'active'
            limit 1
            "#,
            [email.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("user".to_string()))?;

    Ok(User {
        id: row.try_get("", "id")?,
        display_name: row.try_get("", "display_name")?,
        email: row.try_get("", "email")?,
    })
}

async fn find_user_credentials_by_email(
    db: &DatabaseConnection,
    email: &str,
) -> Result<(User, Option<String>), DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id, display_name, email, password_hash
            from users
            where email = $1 and status = 'active'
            limit 1
            "#,
            [email.trim().to_lowercase().into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("user".to_string()))?;

    let user = User {
        id: row.try_get("", "id")?,
        display_name: row.try_get("", "display_name")?,
        email: row.try_get("", "email")?,
    };
    let password_hash = row.try_get("", "password_hash")?;
    Ok((user, password_hash))
}

async fn find_user_by_id(db: &DatabaseConnection, user_id: Uuid) -> Result<User, DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select id, display_name, email
            from users
            where id = $1 and status = 'active'
            limit 1
            "#,
            [user_id.into()],
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("user".to_string()))?;

    Ok(User {
        id: row.try_get("", "id")?,
        display_name: row.try_get("", "display_name")?,
        email: row.try_get("", "email")?,
    })
}

async fn list_workspaces_for_user(
    db: &DatabaseConnection,
    user_id: Uuid,
) -> Result<Vec<Workspace>, DbErr> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            select
              w.id,
              w.name,
              w.workspace_type,
              coalesce(w.description, '') as description,
              wm.role
            from workspaces w
            join workspace_members wm on wm.workspace_id = w.id
            where wm.user_id = $1
              and wm.status = 'active'
              and w.status = 'active'
            order by
              case w.workspace_type when 'personal' then 0 when 'school' then 1 else 2 end,
              w.name
            "#,
            [user_id.into()],
        ))
        .await?;

    rows.into_iter()
        .map(|row| {
            let workspace_type: String = row.try_get("", "workspace_type")?;
            let role: String = row.try_get("", "role")?;
            Ok(Workspace {
                id: row.try_get("", "id")?,
                name: row.try_get("", "name")?,
                workspace_type: parse_workspace_type(&workspace_type),
                role: parse_workspace_role(&role),
                description: row.try_get("", "description")?,
            })
        })
        .collect()
}

fn parse_workspace_type(value: &str) -> WorkspaceType {
    match value {
        "school" => WorkspaceType::School,
        "platform" => WorkspaceType::Platform,
        _ => WorkspaceType::Personal,
    }
}

fn parse_workspace_role(value: &str) -> WorkspaceRole {
    match value {
        "school_admin" => WorkspaceRole::SchoolAdmin,
        "school_teacher" => WorkspaceRole::SchoolTeacher,
        "platform_operator" => WorkspaceRole::PlatformOperator,
        _ => WorkspaceRole::PersonalOwner,
    }
}

fn token_for_user(user_id: Uuid) -> String {
    if let Some(secret) = auth_token_secret() {
        let expires_at = Utc::now().timestamp() + auth_token_ttl_seconds();
        let signature = session_token_signature(&secret, user_id, expires_at);
        return format!("kindleaf-v1:{user_id}:{expires_at}:{signature}");
    }
    format!("{DEV_TOKEN}:{user_id}")
}

pub fn user_id_from_token(token: &str) -> Option<Option<Uuid>> {
    if token == DEV_TOKEN {
        return Some(None);
    }
    let prefix = format!("{DEV_TOKEN}:");
    if let Some(user_id) = token.strip_prefix(&prefix) {
        return Uuid::parse_str(user_id).ok().map(Some);
    }
    user_id_from_signed_token(token).map(Some)
}

fn user_id_from_signed_token(token: &str) -> Option<Uuid> {
    let secret = auth_token_secret()?;
    user_id_from_signed_token_with_secret(token, &secret, Utc::now().timestamp())
}

fn user_id_from_signed_token_with_secret(token: &str, secret: &str, now: i64) -> Option<Uuid> {
    let parts = token.split(':').collect::<Vec<_>>();
    if parts.len() != 4 || parts[0] != "kindleaf-v1" {
        return None;
    }
    let user_id = Uuid::parse_str(parts[1]).ok()?;
    let expires_at = parts[2].parse::<i64>().ok()?;
    if expires_at <= now {
        return None;
    }
    let expected = session_token_signature(&secret, user_id, expires_at);
    constant_time_eq(&expected, parts[3]).then_some(user_id)
}

fn auth_token_secret() -> Option<String> {
    std::env::var("KINDLEAF_AUTH_TOKEN_SECRET")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| value.len() >= 32)
}

fn auth_token_ttl_seconds() -> i64 {
    std::env::var("KINDLEAF_AUTH_TOKEN_TTL_SECONDS")
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(7 * 24 * 60 * 60)
}

fn session_token_signature(secret: &str, user_id: Uuid, expires_at: i64) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts secrets of any size");
    mac.update(user_id.to_string().as_bytes());
    mac.update(b":");
    mac.update(expires_at.to_string().as_bytes());
    let digest = mac.finalize().into_bytes();
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn constant_time_eq(left: &str, right: &str) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.as_bytes()
        .iter()
        .zip(right.as_bytes())
        .fold(0_u8, |acc, (left, right)| acc | (left ^ right))
        == 0
}

fn hash_password(password: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .expect("Argon2 password hashing should succeed")
        .to_string()
}

fn verify_password(password: &str, stored_hash: Option<&str>) -> bool {
    let Some(stored_hash) = stored_hash else {
        return false;
    };
    let parts = stored_hash.split('$').collect::<Vec<_>>();
    if stored_hash.starts_with("$argon2") {
        return PasswordHash::new(stored_hash)
            .ok()
            .and_then(|hash| {
                Argon2::default()
                    .verify_password(password.as_bytes(), &hash)
                    .ok()
            })
            .is_some();
    }
    if parts.len() == 3 && parts[0] == "sha256" {
        return password_digest(parts[1], password) == parts[2];
    }
    stored_hash == password
}

fn password_digest(salt: &str, password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(b":");
    hasher.update(password.as_bytes());
    let digest = hasher.finalize();
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::{
        hash_password, session_token_signature, user_id_from_signed_token_with_secret,
        user_id_from_token, verify_password,
    };
    use uuid::Uuid;

    #[test]
    fn password_hash_round_trips_without_storing_plaintext() {
        let stored = hash_password("password123");
        assert!(stored.starts_with("$argon2id$"));
        assert!(!stored.contains("password123"));
        assert!(verify_password("password123", Some(&stored)));
        assert!(!verify_password("wrong-password", Some(&stored)));
    }

    #[test]
    fn password_verify_keeps_legacy_hashes_and_demo_password_compatible() {
        let legacy_salt = "legacy-salt";
        let legacy_sha256 = format!(
            "sha256${legacy_salt}${}",
            super::password_digest(legacy_salt, "password123")
        );
        assert!(verify_password("password123", Some(&legacy_sha256)));
        assert!(!verify_password("wrong-password", Some(&legacy_sha256)));
        assert!(verify_password("demo", Some("demo")));
        assert!(!verify_password("wrong", Some("demo")));
        assert!(!verify_password("demo", None));
    }

    #[test]
    fn token_parser_accepts_legacy_dev_tokens() {
        let user_id = Uuid::new_v4();
        assert_eq!(user_id_from_token("dev-token"), Some(None));
        assert_eq!(
            user_id_from_token(&format!("dev-token:{user_id}")),
            Some(Some(user_id))
        );
        assert_eq!(user_id_from_token("dev-token:not-a-uuid"), None);
    }

    #[test]
    fn token_signature_is_stable_for_same_payload() {
        let user_id = Uuid::new_v4();
        let signature =
            session_token_signature("a-secret-with-at-least-32-characters", user_id, 42);
        assert_eq!(signature.len(), 64);
        assert_eq!(
            signature,
            session_token_signature("a-secret-with-at-least-32-characters", user_id, 42)
        );
        assert_ne!(
            signature,
            session_token_signature("another-secret-with-32-characters", user_id, 42)
        );
    }

    #[test]
    fn signed_token_parser_rejects_expired_tampered_or_wrong_secret_tokens() {
        let user_id = Uuid::new_v4();
        let secret = "a-secret-with-at-least-32-characters";
        let expires_at = 2_000;
        let signature = session_token_signature(secret, user_id, expires_at);
        let token = format!("kindleaf-v1:{user_id}:{expires_at}:{signature}");

        assert_eq!(
            user_id_from_signed_token_with_secret(&token, secret, 1_999),
            Some(user_id)
        );
        assert_eq!(
            user_id_from_signed_token_with_secret(&token, secret, 2_000),
            None
        );
        assert_eq!(
            user_id_from_signed_token_with_secret(
                &token,
                "another-secret-with-at-least-32-characters",
                1_999
            ),
            None
        );

        let tampered_user_token =
            token.replacen(&user_id.to_string(), &Uuid::new_v4().to_string(), 1);
        assert_eq!(
            user_id_from_signed_token_with_secret(&tampered_user_token, secret, 1_999),
            None
        );

        let tampered_signature_token = format!("kindleaf-v1:{user_id}:{expires_at}:bad-signature");
        assert_eq!(
            user_id_from_signed_token_with_secret(&tampered_signature_token, secret, 1_999),
            None
        );
    }
}
