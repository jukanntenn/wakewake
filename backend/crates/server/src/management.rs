//! Management CLI 命令（Django manage.py 模式）。
//!
//! 复用 lib 层 config + db + repo，零重复。通过 `wakewake-server <subcommand>` 调用。
//! 当前命令：`admin {promote,demote,create,list}`。后续可扩展 agent/device/user 等管理命令。

use sqlx::PgPool;

use crate::config::{AdminCommand, Cli, SecuritySettings, Settings};
use crate::domain::BCRYPT_COST;
use crate::repo::agent_repo;
use crate::repo::user_repo;

/// management 命令入口。最小化 tracing（console only），不启动文件轮转/OTel。
pub async fn run_admin(action: &AdminCommand, cli: &Cli) -> anyhow::Result<()> {
    let settings = Settings::load(cli)?;
    init_minimal_tracing(&settings);
    let pool = crate::db::create_pool(&settings.database.dsn).await?;
    match action {
        AdminCommand::Promote { email } => promote(&pool, email).await,
        AdminCommand::Demote { email } => demote(&pool, email).await,
        AdminCommand::Create { email } => create(&pool, email).await,
        AdminCommand::List => list(&pool).await,
    }
}

/// 启动时确保默认 admin 存在且拥有 agent（幂等）。
///
/// 三分支：
/// - admin 不存在 → 建 user（superuser）+ 默认 agent（与 `auth_service::register` 对称）。
/// - admin 存在但无 agent（老版本创建的 admin）→ 补建 agent（升级补偿）。
/// - admin 存在且有 agent → 无操作。
pub async fn ensure_bootstrap_admin(
    pool: &PgPool,
    security: &SecuritySettings,
) -> anyhow::Result<()> {
    let user = if let Some(u) =
        user_repo::find_by_email(pool, &security.bootstrap_admin_email).await?
    {
        tracing::debug!(
            email = %security.bootstrap_admin_email,
            "bootstrap admin already exists"
        );
        u
    } else {
        let hash = bcrypt::hash(&security.bootstrap_admin_password, BCRYPT_COST)?;
        let user =
            user_repo::insert_verified(pool, &security.bootstrap_admin_email, &hash, true).await?;
        tracing::info!(
            email = %security.bootstrap_admin_email,
            "bootstrap admin created"
        );
        println!(
            "✓ Bootstrap admin created: {}",
            security.bootstrap_admin_email
        );
        user
    };
    // 补偿：admin 存在但无 agent → 补建（覆盖老版本 admin + 保证与普通用户对称）。
    ensure_agent_for_user(pool, user.id, &security.bootstrap_admin_email).await
}

/// 确保指定用户拥有默认 agent；无则补建。幂等。
async fn ensure_agent_for_user(pool: &PgPool, user_id: i64, email: &str) -> anyhow::Result<()> {
    if agent_repo::find_by_user(pool, user_id).await?.is_some() {
        return Ok(());
    }
    crate::service::agent_service::create_default(pool, user_id)
        .await
        .map_err(|e| anyhow::anyhow!("failed to create agent for admin {email}: {e}"))?;
    tracing::info!(email, "bootstrap admin agent created (compensated)");
    println!("✓ Bootstrap admin agent created");
    Ok(())
}

async fn promote(pool: &PgPool, email: &str) -> anyhow::Result<()> {
    let user = user_repo::find_by_email(pool, email)
        .await?
        .ok_or_else(|| anyhow::anyhow!("user not found: {email}"))?;
    if user.is_superuser {
        println!("✓ {email} is already an admin (id={})", user.id);
        return Ok(());
    }
    user_repo::set_superuser(pool, user.id, true).await?;
    ensure_agent_for_user(pool, user.id, email).await?;
    println!("✓ Promoted {email} (id={}) to admin", user.id);
    println!("  ⚠ 该用户需重新登录以获取 admin 权限的 access token。");
    Ok(())
}

async fn demote(pool: &PgPool, email: &str) -> anyhow::Result<()> {
    let user = user_repo::find_by_email(pool, email)
        .await?
        .ok_or_else(|| anyhow::anyhow!("user not found: {email}"))?;
    if !user.is_superuser {
        println!("✓ {email} (id={}) is not an admin", user.id);
        return Ok(());
    }
    let count = user_repo::count_superusers(pool).await?;
    if count <= 1 {
        anyhow::bail!("refuse to demote the last remaining admin (id={})", user.id);
    }
    user_repo::set_superuser(pool, user.id, false).await?;
    println!("✓ Demoted {email} (id={}) from admin", user.id);
    Ok(())
}

async fn create(pool: &PgPool, email: &str) -> anyhow::Result<()> {
    if user_repo::find_by_email(pool, email).await?.is_some() {
        anyhow::bail!("user already exists: {email} (use `admin promote` instead)");
    }
    let password = rpassword::prompt_password("Password: ")?;
    if password.is_empty() {
        anyhow::bail!("password cannot be empty");
    }
    let password_bytes = password.len();
    if password_bytes > crate::domain::PASSWORD_MAX_LEN {
        anyhow::bail!(
            "password too long ({password_bytes} bytes, max {})",
            crate::domain::PASSWORD_MAX_LEN
        );
    }
    if password_bytes < crate::domain::PASSWORD_MIN_LEN {
        anyhow::bail!(
            "password too short ({password_bytes} bytes, min {})",
            crate::domain::PASSWORD_MIN_LEN
        );
    }
    let hash = bcrypt::hash(&password, BCRYPT_COST)?;
    let user = user_repo::insert_verified(pool, email, &hash, true).await?;
    ensure_agent_for_user(pool, user.id, email).await?;
    println!("✓ Admin created: {email} (id={})", user.id);
    println!("  ⚠ 该用户需登录以获取 access token。");
    Ok(())
}

async fn list(pool: &PgPool) -> anyhow::Result<()> {
    let items = user_repo::list_superusers(pool).await?;
    if items.is_empty() {
        println!("(no admin users)");
        return Ok(());
    }
    println!("{:<6} {:<32} {:<22}", "ID", "EMAIL", "CREATED_AT");
    for (id, email, created) in items {
        println!("{id:<6} {email:<32} {created}");
    }
    Ok(())
}

/// 最小化 tracing（console → stderr）。management 命令是短命 CLI，不需要文件轮转。
fn init_minimal_tracing(settings: &Settings) {
    use tracing_subscriber::EnvFilter;
    use tracing_subscriber::fmt;
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&settings.log.level));
    let _ = fmt().with_env_filter(filter).with_target(false).try_init();
}
