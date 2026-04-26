use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum RepoProviderKind {
    Github,
    Azure,
    Generic,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RepoIdentity {
    pub provider: RepoProviderKind,
    pub org: String,
    /// Only meaningful for Azure DevOps.
    pub project: Option<String>,
    pub repo: String,
    pub url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum AuthKind {
    None,
    Pat,
    Oauth,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConnectedRepo {
    pub id: i64,
    pub identity: RepoIdentity,
    pub local_path: String,
    pub branch: String,
    pub auth_kind: AuthKind,
    pub auto_sync: bool,
    pub sync_interval_seconds: u32,
    pub last_synced_at: Option<i64>,
    pub last_sync_error: Option<String>,
}
