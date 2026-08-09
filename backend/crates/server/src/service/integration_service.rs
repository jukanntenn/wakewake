//! Integration 服务：CRUD + schema + 脱敏（device-sync-v3 §5.4 / §8.5）。
//!
//! device-sync-v3：create/update/delete/set_enabled 在**同事务**内 bump `projection_version`。
//! **PATCH config 的 secret 字段合并语义**（§8.5）：缺失/null/`"***"` = 不修改，新密文 = 更新。

use sqlx::PgPool;

use crate::domain::integration::{CreateIntegrationInput, Integration, UpdateIntegrationInput};
use crate::error::{AppError, AppResult, ErrorCode, FieldError};
use crate::hub::CommandDispatcher;
use crate::integrations::ProviderRegistry;
use crate::repo::{agent_repo, integration_repo};
use crate::service::state;

/// secret 字段 PATCH 哨兵值（§8.5）：用户没改时前端回传此值，后端识别为不修改。
const SECRET_SENTINEL: &str = "***";

/// 列用户集成（config 脱敏：secret 字段值替换为 "***"）。
pub async fn list(
    pool: &PgPool,
    registry: &ProviderRegistry,
    user_id: i64,
) -> AppResult<Vec<Integration>> {
    let mut items = integration_repo::list_by_user(pool, user_id)
        .await
        .map_err(AppError::from_repo)?;
    for i in &mut items {
        mask_secret_fields(registry, &i.provider, &mut i.config);
    }
    Ok(items)
}

/// 取 provider schema（GET /integrations/:provider/schema）。
pub fn schema<'a>(
    registry: &'a ProviderRegistry,
    provider: &str,
) -> AppResult<&'a serde_json::Value> {
    registry
        .schema(provider)
        .ok_or(AppError::code(ErrorCode::ProviderNotFound))
}

/// 创建集成：jsonschema 校验 config → 入库 → 同事务 bump version → 推 state refresh。
pub async fn create<D: CommandDispatcher>(
    pool: &PgPool,
    registry: &ProviderRegistry,
    dispatcher: &D,
    user_id: i64,
    input: &CreateIntegrationInput,
) -> AppResult<Integration> {
    if !registry.has_provider(&input.provider) {
        return Err(AppError::code(ErrorCode::ProviderNotFound));
    }
    validate_config(registry, &input.provider, &input.config)?;

    let agent = agent_repo::find_by_user(pool, user_id)
        .await
        .map_err(AppError::from_repo)?
        .ok_or(AppError::code(ErrorCode::AgentNotFound))?;

    let mut tx = pool.begin().await.map_err(AppError::from)?;
    let integration = match integration_repo::insert(
        &mut *tx,
        user_id,
        agent.id,
        &input.provider,
        &input.config,
        input.enabled,
    )
    .await
    {
        Ok(i) => i,
        Err(crate::repo::RepoError::Database(ref e))
            if e.as_database_error()
                .is_some_and(sqlx::error::DatabaseError::is_unique_violation) =>
        {
            // §8.5：重复创建 → 409 INTEGRATION_EXISTS
            return Err(AppError::code(ErrorCode::IntegrationExists));
        },
        Err(e) => return Err(AppError::from_repo(e)),
    };
    let _ = agent_repo::bump_projection_version(&mut *tx, agent.id)
        .await
        .map_err(AppError::from_repo)?;
    tx.commit().await.map_err(AppError::from)?;

    state::refresh_for_agent(pool, dispatcher, agent.id).await?;

    let mut integration = integration;
    mask_secret_fields(registry, &integration.provider, &mut integration.config);
    Ok(integration)
}

/// 获取集成详情（脱敏）。
pub async fn get(
    pool: &PgPool,
    registry: &ProviderRegistry,
    user_id: i64,
    provider: &str,
) -> AppResult<Integration> {
    let mut integration = integration_repo::find_for_user(pool, user_id, provider)
        .await
        .map_err(AppError::from_repo)?
        .ok_or(AppError::code(ErrorCode::IntegrationNotFound))?;
    mask_secret_fields(registry, &integration.provider, &mut integration.config);
    Ok(integration)
}

/// 更新集成 config/enabled（PATCH）。
///
/// device-sync-v3 §8.5 secret 字段合并语义：
/// - 缺失 / null / `"***"` → 不修改（保留 DB 旧密文）
/// - 新密文（base64 RSA）→ 更新
///
/// 实现：读 DB 当前 config → 对 PATCH config 的每个字段应用合并规则 → 写回完整 config。
pub async fn update<D: CommandDispatcher>(
    pool: &PgPool,
    registry: &ProviderRegistry,
    dispatcher: &D,
    user_id: i64,
    provider: &str,
    input: &UpdateIntegrationInput,
) -> AppResult<Integration> {
    let agent = agent_repo::find_by_user(pool, user_id)
        .await
        .map_err(AppError::from_repo)?
        .ok_or(AppError::code(ErrorCode::AgentNotFound))?;

    // 若传了 config，先读 DB 当前值 + 应用 secret 合并规则 → 校验 → 写回
    let merged_config = if let Some(patch) = &input.config {
        let existing = integration_repo::find_for_user(pool, user_id, provider)
            .await
            .map_err(AppError::from_repo)?
            .ok_or(AppError::code(ErrorCode::IntegrationNotFound))?;
        let merged = merge_secret_config(registry, provider, &existing.config, patch);
        validate_config(registry, provider, &merged)?;
        Some(merged)
    } else {
        None
    };

    let mut tx = pool.begin().await.map_err(AppError::from)?;
    let integration = integration_repo::update_config(
        &mut *tx,
        user_id,
        provider,
        merged_config.as_ref(),
        input.enabled,
    )
    .await
    .map_err(AppError::from_repo)?
    .ok_or(AppError::code(ErrorCode::IntegrationNotFound))?;
    let _ = agent_repo::bump_projection_version(&mut *tx, agent.id)
        .await
        .map_err(AppError::from_repo)?;
    tx.commit().await.map_err(AppError::from)?;

    state::refresh_for_agent(pool, dispatcher, agent.id).await?;

    let mut integration = integration;
    mask_secret_fields(registry, &integration.provider, &mut integration.config);
    Ok(integration)
}

/// 删集成：同事务 delete + bump version → 推 state refresh。
pub async fn delete<D: CommandDispatcher>(
    pool: &PgPool,
    dispatcher: &D,
    user_id: i64,
    provider: &str,
) -> AppResult<()> {
    let integration = integration_repo::find_for_user(pool, user_id, provider)
        .await
        .map_err(AppError::from_repo)?
        .ok_or(AppError::code(ErrorCode::IntegrationNotFound))?;
    let agent = agent_repo::find_by_user(pool, user_id)
        .await
        .map_err(AppError::from_repo)?
        .ok_or(AppError::code(ErrorCode::AgentNotFound))?;

    let mut tx = pool.begin().await.map_err(AppError::from)?;
    let deleted = integration_repo::delete(&mut *tx, user_id, provider)
        .await
        .map_err(AppError::from_repo)?;
    if !deleted {
        return Err(AppError::code(ErrorCode::IntegrationNotFound));
    }
    let _ = agent_repo::bump_projection_version(&mut *tx, agent.id)
        .await
        .map_err(AppError::from_repo)?;
    tx.commit().await.map_err(AppError::from)?;

    let _ = integration;
    state::refresh_for_agent(pool, dispatcher, agent.id).await?;
    Ok(())
}

/// 禁用 / 启用。同事务 bump version → 推 state refresh。
pub async fn set_enabled<D: CommandDispatcher>(
    pool: &PgPool,
    dispatcher: &D,
    user_id: i64,
    provider: &str,
    enabled: bool,
) -> AppResult<()> {
    let agent = agent_repo::find_by_user(pool, user_id)
        .await
        .map_err(AppError::from_repo)?
        .ok_or(AppError::code(ErrorCode::AgentNotFound))?;

    let mut tx = pool.begin().await.map_err(AppError::from)?;
    let res = integration_repo::update_config(&mut *tx, user_id, provider, None, Some(enabled))
        .await
        .map_err(AppError::from_repo)?;
    let integration = res.ok_or(AppError::code(ErrorCode::IntegrationNotFound))?;
    let _ = agent_repo::bump_projection_version(&mut *tx, agent.id)
        .await
        .map_err(AppError::from_repo)?;
    tx.commit().await.map_err(AppError::from)?;

    let _ = integration;
    state::refresh_for_agent(pool, dispatcher, agent.id).await?;
    Ok(())
}

/// PATCH config 的 secret 字段合并（§8.5）。
///
/// 以 existing config 为基线，patch 覆盖：
/// - secret 字段：缺失/null/`"***"` → 保留 existing 值；其他（新密文）→ 用 patch 值
/// - 非 secret 字段：patch 有则用 patch 值（缺失则保留 existing）
fn merge_secret_config(
    registry: &ProviderRegistry,
    provider: &str,
    existing: &serde_json::Value,
    patch: &serde_json::Value,
) -> serde_json::Value {
    let mut merged = existing.clone();
    let Some(merged_obj) = merged.as_object_mut() else {
        // existing 非 object（异常），直接用 patch
        return patch.clone();
    };
    let Some(patch_obj) = patch.as_object() else {
        return patch.clone();
    };
    for (key, patch_val) in patch_obj {
        if registry.is_secret_field(provider, key) {
            // secret 字段合并规则
            match patch_val {
                serde_json::Value::Null => {
                    // null → 不修改（保留 existing），跳到下一个 key
                },
                serde_json::Value::String(s) if s == SECRET_SENTINEL => {
                    // "***" → 不修改，跳到下一个 key
                },
                _ => {
                    // 新密文 → 更新
                    merged_obj.insert(key.clone(), patch_val.clone());
                },
            }
        } else {
            // 非 secret 字段：patch 有则覆盖（null 也写入，允许清空非 secret 字段）
            merged_obj.insert(key.clone(), patch_val.clone());
        }
    }
    merged
}

/// jsonschema 校验 config，失败映射为 `VALIDATION_FAILED` + FieldError（config.<path>）。
fn validate_config(
    registry: &ProviderRegistry,
    provider: &str,
    config: &serde_json::Value,
) -> AppResult<()> {
    match registry.validate(provider, config) {
        Ok(()) => Ok(()),
        Err(errors) => {
            let field_errors: Vec<FieldError> = errors
                .into_iter()
                .map(|e| FieldError {
                    field: format!("config.{}", e.path).into(),
                    code: "invalid_format",
                    params: None,
                })
                .collect();
            Err(AppError::validation(field_errors))
        },
    }
}

/// 脱敏：secret 字段值替换为 "***"。按 provider schema 的 secret:true 标记。
fn mask_secret_fields(registry: &ProviderRegistry, provider: &str, config: &mut serde_json::Value) {
    let Some(obj) = config.as_object_mut() else {
        return;
    };
    let fields: Vec<String> = obj.keys().cloned().collect();
    for field in fields {
        if registry.is_secret_field(provider, &field) {
            obj.insert(field, serde_json::Value::String(SECRET_SENTINEL.into()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 测试用 stub registry：bemfa provider，uid/secret_id/secret_key 为 secret 字段。
    fn stub_registry() -> ProviderRegistry {
        ProviderRegistry::build().expect("provider registry build")
    }

    #[test]
    fn merge_secret_keeps_old_when_missing() {
        let reg = stub_registry();
        let existing = json!({"uid": "old-ct", "secret_id": "old-sid", "secret_key": "old-skey"});
        // patch 完全缺失 secret 字段 → 全保留
        let patch = json!({});
        let merged = merge_secret_config(&reg, "bemfa", &existing, &patch);
        assert_eq!(merged["uid"], "old-ct");
        assert_eq!(merged["secret_id"], "old-sid");
    }

    #[test]
    fn merge_secret_keeps_old_when_null() {
        let reg = stub_registry();
        let existing = json!({"uid": "old-ct"});
        // null → 不修改
        let patch = json!({"uid": null});
        let merged = merge_secret_config(&reg, "bemfa", &existing, &patch);
        assert_eq!(merged["uid"], "old-ct");
    }

    #[test]
    fn merge_secret_keeps_old_when_sentinel() {
        let reg = stub_registry();
        let existing = json!({"uid": "old-ct"});
        // "***" → 不修改
        let patch = json!({"uid": "***"});
        let merged = merge_secret_config(&reg, "bemfa", &existing, &patch);
        assert_eq!(merged["uid"], "old-ct");
    }

    #[test]
    fn merge_secret_updates_when_new_ciphertext() {
        let reg = stub_registry();
        let existing = json!({"uid": "old-ct"});
        // 新密文 → 更新
        let patch = json!({"uid": "new-ct-base64"});
        let merged = merge_secret_config(&reg, "bemfa", &existing, &patch);
        assert_eq!(merged["uid"], "new-ct-base64");
    }

    #[test]
    fn mask_secrets_replaces_with_sentinel() {
        let reg = stub_registry();
        let mut config = json!({"uid": "real-ct", "secret_id": "real-sid"});
        mask_secret_fields(&reg, "bemfa", &mut config);
        assert_eq!(config["uid"], "***");
        assert_eq!(config["secret_id"], "***");
    }
}
