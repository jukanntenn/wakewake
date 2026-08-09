//! 集成 provider 注册表：各 provider 的 schema 定义（schema-as-data）。
//!
//! 同一份 JSON Schema（Draft 2020-12）驱动三处（new-design.md §5.2 integrations）：
//! (1) 启动时编译为 `jsonschema::Validator` 做后端校验；
//! (2) GET /integrations/:provider/schema 返回给前端驱动表单渲染；
//! (3) secret:true 扩展 keyword 标注敏感字段（API 返回脱敏为 "***"）。

pub mod bemfa;

use std::collections::HashMap;
use std::sync::Arc;

/// secret:true 扩展 keyword（非 JSON Schema 标准，自定义）：
/// 标注的字段（如 Bemfa uid）前端 RSA 加密提交，API 返回 "***"，agent 持私钥解密。
pub const SECRET_KEYWORD: &str = "secret";

/// 预编译的 provider schema 集合（启动时编译一次，请求时复用）。
#[derive(Clone)]
pub struct ProviderRegistry {
    schemas: Arc<HashMap<String, serde_json::Value>>,
    validators: Arc<HashMap<String, jsonschema::Validator>>,
}

impl ProviderRegistry {
    /// 启动时编译所有 provider schema。
    pub fn build() -> Result<Self, jsonschema::ValidationError<'static>> {
        let providers = [bemfa::schema()];
        let mut schemas = HashMap::new();
        let mut validators = HashMap::new();
        for (name, schema) in providers {
            // Draft 2020-12 下 format 默认是 annotation，需显式开启校验。
            let validator = jsonschema::options()
                .should_validate_formats(true)
                .build(&schema)?;
            schemas.insert((*name).to_string(), schema);
            validators.insert((*name).to_string(), validator);
        }
        Ok(Self {
            schemas: Arc::new(schemas),
            validators: Arc::new(validators),
        })
    }

    #[must_use]
    pub fn has_provider(&self, name: &str) -> bool {
        self.schemas.contains_key(name)
    }

    #[must_use]
    pub fn schema(&self, name: &str) -> Option<&serde_json::Value> {
        self.schemas.get(name)
    }

    /// 校验用户提交的 `config；返全部错误（instance_path` 映射 `FieldError`）。
    pub fn validate<'a>(
        &'a self,
        provider: &str,
        config: &'a serde_json::Value,
    ) -> Result<(), Vec<ValidationError>> {
        let validator = match self.validators.get(provider) {
            Some(v) => v,
            None => return Err(vec![ValidationError::provider_not_found()]),
        };
        let errors: Vec<_> = validator.iter_errors(config).collect();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors
                .into_iter()
                .map(|e| ValidationError {
                    // instance_path 返 "/uid"，strip 前导 / 拼 config. 前缀（error-handling.md §4.1）
                    path: e
                        .instance_path()
                        .as_str()
                        .trim_start_matches('/')
                        .to_string(),
                    message: e.to_string(),
                })
                .collect())
        }
    }

    /// 标记字段是否敏感（脱敏用）。读 schema 的 secret:true keyword。
    #[must_use]
    pub fn is_secret_field(&self, provider: &str, field: &str) -> bool {
        let schema = match self.schemas.get(provider) {
            Some(s) => s,
            None => return false,
        };
        schema
            .get("properties")
            .and_then(|p| p.get(field))
            .and_then(|f| f.get(SECRET_KEYWORD))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone)]
pub struct ValidationError {
    pub path: String,
    pub message: String,
}

impl ValidationError {
    fn provider_not_found() -> Self {
        Self {
            path: "provider".to_string(),
            message: "unknown provider".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 合法 32 位 hex UID（v1/v2 路由测试复用）。
    const VALID_UID_32: &str = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4";

    #[test]
    fn registry_builds_and_validates_bemfa() {
        let registry = ProviderRegistry::build().expect("build");
        assert!(registry.has_provider("bemfa"));

        // 合法 config（v1：仅 uid；topic_prefix 已移除，由代码常量 ww 固定）
        let valid = serde_json::json!({"uid": VALID_UID_32});
        assert!(registry.validate("bemfa", &valid).is_ok());

        // 缺 uid
        let invalid = serde_json::json!({});
        assert!(registry.validate("bemfa", &invalid).is_err());

        // topic_prefix 已从 schema 移除——提交它应被 additionalProperties:false 拒绝
        assert!(
            registry
                .validate(
                    "bemfa",
                    &serde_json::json!({"uid": VALID_UID_32, "topic_prefix": "anything"})
                )
                .is_err()
        );

        // secret 字段标记：uid/secret_id/secret_key 均 secret
        assert!(registry.is_secret_field("bemfa", "uid"));
        assert!(registry.is_secret_field("bemfa", "secret_id"));
        assert!(registry.is_secret_field("bemfa", "secret_key"));
    }

    #[test]
    fn uid_accepted_as_any_string_secret_encrypted() {
        // uid 标 secret:true，前端 RSA 加密提交——server 永不可见明文，故 schema 不加
        // pattern/minLength（密文无法通过明文格式校验）。任何非空字符串均通过 server 校验；
        // 明文质量校验（32 hex / 45 char）由 agent 解密后做（见 bemfa_state::is_valid_uid）。
        let registry = ProviderRegistry::build().expect("build");
        // 明文合法值（agent 侧会通过）
        assert!(
            registry
                .validate("bemfa", &serde_json::json!({"uid": VALID_UID_32}))
                .is_ok()
        );
        // 密文模拟（base64 RSA，含 +/= 等非 hex 字符）也必须通过 server 校验
        assert!(
            registry
                .validate(
                    "bemfa",
                    &serde_json::json!({"uid": "ciphertext-with+/=and-special.chars"})
                )
                .is_ok()
        );
        // 短字符串（模拟明文，但 server 不应拦）也通过——agent 侧拦
        assert!(
            registry
                .validate("bemfa", &serde_json::json!({"uid": "abc"}))
                .is_ok()
        );
    }

    #[test]
    fn schema_secret_pair_both_present_ok() {
        // v2：secret_id + secret_key 都填 → 通过
        let registry = ProviderRegistry::build().expect("build");
        assert!(
            registry
                .validate(
                    "bemfa",
                    &serde_json::json!({"uid": VALID_UID_32, "secret_id": "sid", "secret_key": "skey"})
                )
                .is_ok()
        );
    }

    #[test]
    fn schema_secret_pair_none_present_ok() {
        // v1：都不填 → 通过
        let registry = ProviderRegistry::build().expect("build");
        assert!(
            registry
                .validate("bemfa", &serde_json::json!({"uid": VALID_UID_32}))
                .is_ok()
        );
    }

    #[test]
    fn schema_secret_id_without_key_rejected() {
        // 只填 secret_id 不填 secret_key → 拒绝（if/then 成对校验）
        let registry = ProviderRegistry::build().expect("build");
        assert!(
            registry
                .validate(
                    "bemfa",
                    &serde_json::json!({"uid": VALID_UID_32, "secret_id": "sid"})
                )
                .is_err()
        );
    }

    #[test]
    fn schema_secret_key_without_id_rejected() {
        // 只填 secret_key 不填 secret_id → 拒绝（对称校验）
        let registry = ProviderRegistry::build().expect("build");
        assert!(
            registry
                .validate(
                    "bemfa",
                    &serde_json::json!({"uid": VALID_UID_32, "secret_key": "skey"})
                )
                .is_err()
        );
    }
}
