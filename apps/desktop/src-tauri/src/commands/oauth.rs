//! GitHub OAuth Device Flow.
//!
//! The user pastes the displayed code into github.com/login/device, and the
//! frontend polls `github_device_poll` until the access token comes back.
//!
//! GitHub's OAuth Apps require a client ID that is **not secret**; we ship it
//! as a constant. See https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps#device-flow

use serde::{Deserialize, Serialize};

/// Senda's public OAuth App client id. The user can override this from the
/// frontend if their org uses a different application.
const DEFAULT_CLIENT_ID: &str = "Iv23liGKv4fJSSenda00";

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Deserialize)]
struct GithubDeviceResp {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DeviceTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub scope: String,
}

#[derive(Debug, Deserialize)]
struct GithubTokenResp {
    access_token: Option<String>,
    token_type: Option<String>,
    scope: Option<String>,
    error: Option<String>,
    #[allow(dead_code)]
    error_description: Option<String>,
}

#[tauri::command]
pub async fn github_device_authorize(
    client_id: Option<String>,
) -> Result<DeviceCodeResponse, String> {
    let client_id = client_id.unwrap_or_else(|| DEFAULT_CLIENT_ID.to_string());
    let resp: GithubDeviceResp = reqwest::Client::new()
        .post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", client_id.as_str()),
            ("scope", "repo read:user"),
        ])
        .send()
        .await
        .map_err(|e| format!("device/code: {e}"))?
        .error_for_status()
        .map_err(|e| format!("device/code: {e}"))?
        .json()
        .await
        .map_err(|e| format!("device/code: {e}"))?;

    Ok(DeviceCodeResponse {
        device_code: resp.device_code,
        user_code: resp.user_code,
        verification_uri: resp.verification_uri,
        expires_in: resp.expires_in,
        interval: resp.interval,
    })
}

#[tauri::command]
pub async fn github_device_poll(
    device_code: String,
    client_id: Option<String>,
) -> Result<Option<DeviceTokenResponse>, String> {
    let client_id = client_id.unwrap_or_else(|| DEFAULT_CLIENT_ID.to_string());
    let resp: GithubTokenResp = reqwest::Client::new()
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", client_id.as_str()),
            ("device_code", device_code.as_str()),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .send()
        .await
        .map_err(|e| format!("token: {e}"))?
        .json()
        .await
        .map_err(|e| format!("token: {e}"))?;

    if let Some(err) = resp.error {
        // The expected "still waiting" responses are returned as Ok(None) so the
        // frontend can keep polling on the same interval without raising.
        return match err.as_str() {
            "authorization_pending" | "slow_down" => Ok(None),
            other => Err(other.to_string()),
        };
    }

    let access_token = resp
        .access_token
        .ok_or_else(|| "no access_token".to_string())?;
    Ok(Some(DeviceTokenResponse {
        access_token,
        token_type: resp.token_type.unwrap_or_default(),
        scope: resp.scope.unwrap_or_default(),
    }))
}
