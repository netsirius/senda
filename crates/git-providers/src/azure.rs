use async_trait::async_trait;
use std::path::{Path, PathBuf};
use url::Url;

use senda_core::RepoProviderKind;

use crate::{
    Auth, AzureProvider, Branch, PrInfo, PrRequest, ProviderError, PullResult, RepoIdentity,
    RepoProvider,
};

#[async_trait]
impl RepoProvider for AzureProvider {
    fn provider_name(&self) -> &str {
        "azure"
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
        // Expected layouts:
        //   https://dev.azure.com/{org}/{project}/_git/{repo}
        //   https://{org}.visualstudio.com/{project}/_git/{repo}
        let host = parsed.host_str().unwrap_or_default();
        let (org, project, repo) = if host == "dev.azure.com" {
            if segments.len() < 4 || segments[2] != "_git" {
                return Err(ProviderError::InvalidUrl(raw.to_string()));
            }
            (
                segments[0].to_string(),
                segments[1].to_string(),
                segments[3].to_string(),
            )
        } else if host.ends_with(".visualstudio.com") {
            let org = host.trim_end_matches(".visualstudio.com").to_string();
            if segments.len() < 3 || segments[1] != "_git" {
                return Err(ProviderError::InvalidUrl(raw.to_string()));
            }
            (org, segments[0].to_string(), segments[2].to_string())
        } else {
            return Err(ProviderError::InvalidUrl(raw.to_string()));
        };

        Ok(RepoIdentity {
            provider: RepoProviderKind::Azure,
            org,
            project: Some(project),
            repo: repo.trim_end_matches(".git").to_string(),
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
    async fn parses_dev_azure_com_url() {
        let provider = AzureProvider;
        let identity = provider
            .parse_url("https://dev.azure.com/acme/platform/_git/agents")
            .await
            .unwrap();
        assert_eq!(identity.org, "acme");
        assert_eq!(identity.project.as_deref(), Some("platform"));
        assert_eq!(identity.repo, "agents");
    }

    #[tokio::test]
    async fn parses_legacy_visualstudio_url() {
        let provider = AzureProvider;
        let identity = provider
            .parse_url("https://acme.visualstudio.com/platform/_git/agents")
            .await
            .unwrap();
        assert_eq!(identity.org, "acme");
        assert_eq!(identity.project.as_deref(), Some("platform"));
        assert_eq!(identity.repo, "agents");
    }
}
