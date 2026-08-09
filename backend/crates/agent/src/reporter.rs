//! Reporter：POST complete/sync/wakes 到 server（device-sync-v3 §8.6）。
//!
//! - `complete`（POST .../commands/{id}/complete）：wake 命令终态回报，200。
//! - `sync`（POST .../sync）：观测/ack 同步，200 + current_version（I6 守卫支撑）。
//! - `wake`（POST .../wakes）：MQTT 触发的语音唤醒上报，201。

use reqwest::Client;
use wakewake_protocol::{CommandResult, SyncRequest, SyncResponse, WakeRequest, WakeResponse};

#[derive(Debug, thiserror::Error)]
pub enum ReportError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("server returned {0}")]
    Status(u16),
}

/// POST `/agents/self/commands/{command_id}/complete`。
pub async fn complete(
    client: &Client,
    server_url: &str,
    pairing_code: &str,
    command_id: &str,
    result: CommandResult,
) -> Result<(), ReportError> {
    let url = format!("{server_url}/api/v1/agents/self/commands/{command_id}/complete");
    let resp = client
        .post(&url)
        .bearer_auth(pairing_code)
        .json(&result)
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(ReportError::Status(resp.status().as_u16()));
    }
    Ok(())
}

/// POST `/agents/self/sync`（§8.6 上行 2）。返回 current_version（I6 守卫支撑）。
pub async fn sync(
    client: &Client,
    server_url: &str,
    pairing_code: &str,
    req: SyncRequest,
) -> Result<SyncResponse, ReportError> {
    let url = format!("{server_url}/api/v1/agents/self/sync");
    let resp = client
        .post(&url)
        .bearer_auth(pairing_code)
        .json(&req)
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(ReportError::Status(resp.status().as_u16()));
    }
    let sync_resp = resp.json::<SyncResponse>().await?;
    Ok(sync_resp)
}

/// POST `/agents/self/wakes`（§8.6 上行 3，MQTT 触发的语音唤醒上报）。
pub async fn wake(
    client: &Client,
    server_url: &str,
    pairing_code: &str,
    req: WakeRequest,
) -> Result<WakeResponse, ReportError> {
    let url = format!("{server_url}/api/v1/agents/self/wakes");
    let resp = client
        .post(&url)
        .bearer_auth(pairing_code)
        .json(&req)
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(ReportError::Status(resp.status().as_u16()));
    }
    let wake_resp = resp.json::<WakeResponse>().await?;
    Ok(wake_resp)
}
