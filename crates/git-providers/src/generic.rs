use async_trait::async_trait;
use std::path::{Path, PathBuf};
use url::Url;

use senda_core::RepoProviderKind;

use crate::{
    Auth, Branch, GenericProvider, PrInfo, PrRequest, ProviderError, PullResult, RepoIdentity,
    RepoProvider,
};

#[async_trait]
impl RepoProvider for GenericProvider {
    fn provider_name(&self) -> &str {
        "generic"
    }

    /// Generic git has no standard PR API, so [`PublishFlow`] degrades to a
    /// "push the branch and open the PR yourself" message.
    fn supports_pr_creation(&self) -> bool {
        false
    }

    async fn parse_url(&self, raw: &str) -> Result<RepoIdentity, ProviderError> {
        let parsed = Url::parse(raw).map_err(|_| ProviderError::InvalidUrl(raw.to_string()))?;
        let host = parsed.host_str().unwrap_or_default().to_string();
        let segments: Vec<_> = parsed
            .path_segments()
            .map(|p| p.filter(|s| !s.is_empty()).collect())
            .unwrap_or_default();
        if segments.is_empty() {
            return Err(ProviderError::InvalidUrl(raw.to_string()));
        }
        let repo = segments
            .last()
            .map(|s| s.trim_end_matches(".git").to_string())
            .unwrap_or_default();
        let org = if segments.len() >= 2 {
            segments[..segments.len() - 1].join("/")
        } else {
            host.clone()
        };
        Ok(RepoIdentity {
            provider: RepoProviderKind::Generic,
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
        Err(ProviderError::Unsupported)
    }

    async fn list_branches(&self, _repo: &Path) -> Result<Vec<Branch>, ProviderError> {
        Err(ProviderError::NotImplemented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn extracts_repo_from_gitlab_url() {
        let provider = GenericProvider;
        let identity = provider
            .parse_url("https://gitlab.com/team/agents")
            .await
            .unwrap();
        assert_eq!(identity.repo, "agents");
        assert_eq!(identity.org, "team");
    }
}
