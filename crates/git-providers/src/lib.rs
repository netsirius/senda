//! Repo providers — trait + implementations.
//!
//! Phase 0 keeps this crate as a typed skeleton: the trait is final, the
//! provider detection from URL is implemented with tests, and the three
//! provider structs are unimplemented stubs returning [`ProviderError::NotImplemented`].
//! Phase 2 fills in the real network code (octocrab, Azure REST, git2).

use async_trait::async_trait;
use std::path::{Path, PathBuf};
use thiserror::Error;
use url::Url;

pub use senda_core::{RepoIdentity, RepoProviderKind};

#[derive(Debug, Clone)]
pub enum Auth {
    None,
    PersonalAccessToken(String),
    DeviceFlow { access_token: String },
}

#[derive(Debug, Clone)]
pub struct PrRequest {
    pub title: String,
    pub body: String,
    pub base: String,
    pub head: String,
    pub reviewers: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PrInfo {
    pub url: String,
    pub number: u32,
}

#[derive(Debug, Clone)]
pub struct PullResult {
    pub updated: bool,
    pub commits_pulled: u32,
}

#[derive(Debug, Clone)]
pub struct Branch {
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("invalid repository URL: {0}")]
    InvalidUrl(String),

    #[error("provider does not support this operation")]
    Unsupported,

    #[error("provider not implemented yet")]
    NotImplemented,

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[async_trait]
pub trait RepoProvider: Send + Sync {
    fn provider_name(&self) -> &str;

    fn supports_pr_creation(&self) -> bool;

    async fn parse_url(&self, url: &str) -> Result<RepoIdentity, ProviderError>;

    async fn clone(
        &self,
        identity: &RepoIdentity,
        dest: &Path,
        auth: &Auth,
    ) -> Result<PathBuf, ProviderError>;

    async fn pull(&self, repo: &Path) -> Result<PullResult, ProviderError>;

    async fn push(&self, repo: &Path, branch: &str, auth: &Auth) -> Result<(), ProviderError>;

    async fn create_pr(
        &self,
        repo: &RepoIdentity,
        pr: PrRequest,
        auth: &Auth,
    ) -> Result<PrInfo, ProviderError>;

    async fn list_branches(&self, repo: &Path) -> Result<Vec<Branch>, ProviderError>;
}

pub struct GitHubProvider;
pub struct AzureProvider;
pub struct GenericProvider;

/// Detect which provider owns a repository URL. Falls back to [`GenericProvider`]
/// when the host is not a known SaaS provider (gitlab, bitbucket, self-hosted, …).
pub fn detect_provider(url: &str) -> Box<dyn RepoProvider> {
    let host = Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|s| s.to_lowercase()))
        .unwrap_or_default();

    if host == "github.com" || host.ends_with(".github.com") {
        Box::new(GitHubProvider)
    } else if host == "dev.azure.com" || host.ends_with(".visualstudio.com") {
        Box::new(AzureProvider)
    } else {
        Box::new(GenericProvider)
    }
}

mod azure;
mod generic;
mod github;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_url_routes_to_github_provider() {
        let provider = detect_provider("https://github.com/foo/bar");
        assert_eq!(provider.provider_name(), "github");
    }

    #[test]
    fn azure_url_routes_to_azure_provider() {
        let provider = detect_provider("https://dev.azure.com/myorg/proj/_git/repo");
        assert_eq!(provider.provider_name(), "azure");
    }

    #[test]
    fn gitlab_routes_to_generic_provider() {
        let provider = detect_provider("https://gitlab.com/foo/bar");
        assert_eq!(provider.provider_name(), "generic");
    }
}
