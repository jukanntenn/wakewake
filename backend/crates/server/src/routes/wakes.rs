//! 唤醒历史路由（api-design.md §1.4 唤醒历史）。JWT 域。
//!
//! GET /wakes（游标分页 ?before=<`created_at>&page_size=20`）。

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use time::OffsetDateTime;

use crate::error::AppResult;
use crate::middleware::auth::AuthUser;
use crate::repo::wake_repo;
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/wakes", get(list))
}

#[derive(Debug, Deserialize)]
struct WakeQuery {
    before: Option<String>,
    page_size: Option<i64>,
}

async fn list(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Query(q): Query<WakeQuery>,
) -> AppResult<Json<serde_json::Value>> {
    use time::format_description::well_known::Rfc3339;
    let page_size = q.page_size.unwrap_or(20).clamp(1, 100);
    let before = q
        .before
        .as_deref()
        .and_then(|s| OffsetDateTime::parse(s, &Rfc3339).ok());

    let wakes = wake_repo::list_for_user(&state.pool, user.user_id, before, page_size)
        .await
        .map_err(crate::error::AppError::from_repo)?;
    let total = wake_repo::count_for_user(&state.pool, user.user_id, before)
        .await
        .map_err(crate::error::AppError::from_repo)?;

    let items: Vec<_> = wakes
        .iter()
        .map(|w| {
            json!({
                "id": w.id,
                "device_id": w.device_did.simple().to_string(),
                "device_name": w.device_name,
                "type": match w.r#type {
                    crate::domain::wake::WakeType::Wol => "wol",
                    crate::domain::wake::WakeType::BemfaWake => "bemfa_wake",
                },
                "status": w.status.as_str(),
                "message": w.message,
                "created_at": w.created_at.format(&Rfc3339).unwrap_or_default(),
            })
        })
        .collect();
    Ok(Json(json!({
        "items": items,
        "page_size": page_size,
        "total": total,
    })))
}
