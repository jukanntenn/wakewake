//! Bemfa（巴法云）provider schema（Draft 2020-12）。
//!
//! `secret:true` 字段（uid/secret_id/secret_key）前端 RSA 加密提交，server 永不可见
//! 明文——故这些字段仅约束 `type: string`，不加 pattern/minLength（密文无法通过明文
//! 格式校验）。明文质量校验（uid 格式、secret 成对非空）由 agent 解密后做，失败上报
//! IntegrationStatus 写回 last_error。
//!
//! topic 前缀是代码固定的常量（见 `agent::bemfa::TOPIC_PREFIX`），不暴露给用户——
//! 故 schema 中不含 topic_prefix 字段。

use serde_json::{Value, json};

/// provider 名。
pub const NAME: &str = "bemfa";

/// Bemfa JSON Schema（同时驱动后端校验 + 前端表单渲染 + API 脱敏）。
/// `serde_json::Value` 需运行时构造（json! 宏含堆分配，非 const）。
///
/// v1/v2 接口字段驱动路由：仅填 uid → agent 用 v1 createTopic；
/// 额外填 secret_id/secret_key（v2 实名认证凭证）→ agent 用 v2 createTopic。
/// `allOf` if/then 保证 secret_id/secret_key 成对（要么都不填，要么都填）。
#[must_use]
pub fn schema() -> (&'static str, Value) {
    (
        NAME,
        json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "title": "Bemfa",
            "description": "巴法云 IoT MQTT 集成",
            "type": "object",
            "properties": {
                "uid": {
                    "type": "string",
                    "title": "Bemfa UID",
                    "description": "巴法云用户私钥（敏感，端到端加密）。两种格式：32 位十六进制（新版）或 45 位字符（旧版）。同时用于 MQTT 连接（9503 TLS，uid 作 client_id）与 topic API（v1/v2 createTopic/deleteTopic）。",
                    "secret": true
                },
                "secret_id": {
                    "type": "string",
                    "title": "Bemfa secretID（v2 实名认证，可选）",
                    "description": "v2 API 凭证（敏感，端到端加密）。完成巴法云实名认证后在「API 密钥」页获取。与 secret_key 成对填写；填了则启用 v2 接口，否则默认 v1。",
                    "secret": true
                },
                "secret_key": {
                    "type": "string",
                    "title": "Bemfa secretKey（v2，与 secretID 成对）",
                    "description": "v2 API 凭证（敏感，端到端加密）。与 secret_id 成对填写。",
                    "secret": true
                }
            },
            "required": ["uid"],
            "allOf": [
                {
                    "if": { "required": ["secret_id"] },
                    "then": { "required": ["secret_key"] }
                },
                {
                    "if": { "required": ["secret_key"] },
                    "then": { "required": ["secret_id"] }
                }
            ],
            "additionalProperties": false
        }),
    )
}
