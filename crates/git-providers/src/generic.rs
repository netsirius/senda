use async_trait::async_trait;
use std::path::{Path, PathBuf};
use url::Url;

use senda_core::RepoProviderKind;

use crate::{
    git_ops, Auth, Branch, GenericProvider, PrInfo, PrRequest, ProviderError, PullResult,
    RepoIdentity, RepoProvider,
};

#[async_trait]
impl RepoProvider for GenericProvider {
    fn provider_name(&self) -> &str {
        "generic"
    }

    /// Generic Git has no standard PR API. The publish flow degrades to
    /// "push the branch and open the PR yourself" when this returns false.
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
        identity: &RepoIdentity,
        dest: &Path,
        auth: &Auth,
    ) -> Result<PathBuf, ProviderError> {
        let url = identity.url.clone();
        let dest = dest.to_path_buf();
        let auth = auth.clone();
        tokio::task::spawn_blocking(move || git_ops::clone_repo(&url, &dest, auth))
            .await
            .map_err(|e| ProviderError::Other(e.into()))?
    }

    async fn pull(&self, repo: &Path) -> Result<PullResult, ProviderError> {
        let repo = repo.to_path_buf();
        tokio::task::spawn_blocking(move || git_ops::pull_repo(&repo, Auth::None))
            .await
            .map_err(|e| ProviderError::Other(e.into()))?
    }

    async fn push(&self, repo: &Path, branch: &str, auth: &Auth) -> Result<(), ProviderError> {
        let repo = repo.to_path_buf();
        let branch = branch.to_string();
        let auth = auth.clone();
        tokio::task::spawn_blocking(move || git_ops::push_repo(&repo, &branch, auth))
            .await
            .map_err(|e| ProviderError::Other(e.into()))?
    }

    async fn create_pr(
        &self,
        _repo: &RepoIdentity,
        _pr: PrRequest,
        _auth: &Auth,
    ) -> Result<PrInfo, ProviderError> {
        Err(ProviderError::Unsupported)
    }

    async fn list_branches(&self, repo: &Path) -> Result<Vec<Branch>, ProviderError> {
        let repo = repo.to_path_buf();
        tokio::task::spawn_blocking(move || git_ops::list_local_branches(&repo))
            .await
            .map_err(|e| ProviderError::Other(e.into()))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn extracts_repo_from_gitlab_url() {
        let identity = GenericProvider
            .parse_url("https://gitlab.com/team/agents")
            .await
            .unwrap();
        assert_eq!(identity.repo, "agents");
        assert_eq!(identity.org, "team");
    }
}
