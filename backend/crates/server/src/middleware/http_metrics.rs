//! HTTP 请求时长直方图（`http_request_duration_seconds`，observability.rs 可观测 RFC）。
//!
//! 仪表早已在 `observability::metrics` 声明，这里补上记录点：route_layer 挂载
//! （路由已匹配，`MatchedPath` 可用），path 标签取路由模板（`/devices/{id}`）
//! 而非原始 URI——ID 进标签会把序列基数打爆。全局限流（governor）在本层
//! 之外，429 不经过这里；429 的量另有 `http_rate_limited_total` 承载。

use std::time::Instant;

use axum::extract::{MatchedPath, Request};
use axum::http::Method;
use axum::middleware::Next;
use axum::response::Response;

/// 记录 (method, 路由模板, status) 维度的请求处理时长。
pub async fn http_metrics(
    method: Method,
    matched_path: Option<MatchedPath>,
    req: Request,
    next: Next,
) -> Response {
    let start = Instant::now();
    let path = matched_path.map_or_else(|| req.uri().path().to_owned(), |p| p.as_str().to_owned());
    let resp = next.run(req).await;
    crate::observability::metrics::record_http_duration(
        start.elapsed().as_secs_f64(),
        method.as_str(),
        &path,
        resp.status().as_u16(),
    );
    resp
}
