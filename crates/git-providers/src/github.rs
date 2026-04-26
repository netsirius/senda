use async_trait::async_trait;
use std::path::{Path, PathBuf};
use url::Url;

use senda_core::RepoProviderKind;

use crate::{
    Auth, Branch, GitHubProvider, PrInfo, PrRequest, ProviderError, PullResult, RepoIdentity,
    RepoProvider,
};

#[async_trait]
impl RepoProvider for GitHubProvider {
    fn provider_name(&self) -> &str {
        "github"
    }

    fn supports_pr_creation(&self) -> bool {
        true
    }

    async fn parse_url(&self, raw: &str) -> Result<RepoIdentity, ProviderError> {
        let parsed = Url::parse(raw).map_err(|_| ProviderError::InvalidUrl(raw.to_string()))?;
        let segments: Vec<_> = parsed
            .path_segments()
            .map(|p| p.filter(|s| !s.is_empty()).collect())
            .unwrap_or_default();
        if segments.len() < 2 {
            return Err(ProviderError::InvalidUrl(raw.to_string()));
        }
        let org = segments[0].to_string();
        let repo = segments[1].trim_end_matches(".git").to_string();
        Ok(RepoIdentity {
            provider: RepoProviderKind::Github,
            org,
            project: None,
            repo,
            url: raw.to_string(),
        })
    }

    async fn clone(
        &self,
        _identity: &RepoIdentity,
        _dest: &Path,
        _auth: &Auth,
    ) -> Result<PathBuf, ProviderError> {
        Err(ProviderError::NotImplemented)
    }

    async fn pull(&self, _repo: &Path) -> Result<PullResult, ProviderError> {
        Err(ProviderError::NotImplemented)
    }

    async fn push(&self, _repo: &Path, _branch: &str, _auth: &Auth) -> Result<(), ProviderError> {
        Err(ProviderError::NotImplemented)
    }

    async fn create_pr(
        &self,
        _repo: &RepoIdentity,
        _pr: PrRequest,
        _auth: &Auth,
    ) -> Result<PrInfo, ProviderError> {
        Err(ProviderError::NotImplemented)
    }

    async fn list_branches(&self, _repo: &Path) -> Result<Vec<Branch>, ProviderError> {
        Err(ProviderError::NotImplemented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn parses_well_formed_github_url() {
        let provider = GitHubProvider;
        let identity = provider
            .parse_url("https://github.com/freenet/freenet-core")
            .await
            .unwrap();
        assert_eq!(identity.org, "freenet");
        assert_eq!(identity.repo, "freenet-core");
        assert!(identity.project.is_none());
    }

    #[tokio::test]
    async fn strips_dot_git_suffix() {
        let provider = GitHubProvider;
        let identity = provider
            .parse_url("https://github.com/foo/bar.git")
            .await
            .unwrap();
        assert_eq!(identity.repo, "bar");
    }

    #[tokio::test]
    async fn rejects_url_without_repo() {
        let provider = GitHubProvider;
        let err = provider.parse_url("https://github.com/foo").await;
        assert!(matches!(err, Err(ProviderError::InvalidUrl(_))));
    }
}
