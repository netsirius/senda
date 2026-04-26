use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Serialize;
use std::path::{Path, PathBuf};
use url::Url;

use senda_core::RepoProviderKind;

use crate::{
    git_ops, http::client, http::map_reqwest, Auth, AzureProvider, Branch, PrInfo, PrRequest,
    ProviderError, PullResult, RepoIdentity, RepoProvider,
};

#[derive(Serialize)]
struct AzPrBody<'a> {
    #[serde(rename = "sourceRefName")]
    source_ref: String,
    #[serde(rename = "targetRefName")]
    target_ref: String,
    title: &'a str,
    description: &'a str,
}

#[derive(serde::Deserialize)]
struct AzPrResponse {
    #[serde(rename = "pullRequestId")]
    pull_request_id: u32,
    #[serde(rename = "_links")]
    links: AzLinks,
}

#[derive(serde::Deserialize)]
struct AzLinks {
    web: AzLink,
}

#[derive(serde::Deserialize)]
struct AzLink {
    href: String,
}

fn pat_basic_header(pat: &str) -> String {
    let raw = format!(":{pat}");
    format!("Basic {}", STANDARD.encode(raw))
}

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
        repo: &RepoIdentity,
        pr: PrRequest,
        auth: &Auth,
    ) -> Result<PrInfo, ProviderError> {
        let pat = match auth {
            Auth::PersonalAccessToken(t) => t.clone(),
            _ => return Err(ProviderError::Unsupported),
        };
        let project = repo.project.as_deref().ok_or(ProviderError::Unsupported)?;
        let endpoint = format!(
            "https://dev.azure.com/{}/{}/_apis/git/repositories/{}/pullrequests?api-version=7.0",
            repo.org, project, repo.repo
        );
        let body = AzPrBody {
            source_ref: format!("refs/heads/{}", pr.head),
            target_ref: format!("refs/heads/{}", pr.base),
            title: &pr.title,
            description: &pr.body,
        };
        let resp: AzPrResponse = client()
            .post(&endpoint)
            .header("Authorization", pat_basic_header(&pat))
            .json(&body)
            .send()
            .await
            .map_err(map_reqwest)?
            .error_for_status()
            .map_err(map_reqwest)?
            .json()
            .await
            .map_err(map_reqwest)?;

        Ok(PrInfo {
            url: resp.links.web.href,
            number: resp.pull_request_id,
        })
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
    async fn parses_dev_azure_com_url() {
        let identity = AzureProvider
            .parse_url("https://dev.azure.com/acme/platform/_git/agents")
            .await
            .unwrap();
        assert_eq!(identity.org, "acme");
        assert_eq!(identity.project.as_deref(), Some("platform"));
        assert_eq!(identity.repo, "agents");
    }

    #[tokio::test]
    async fn parses_legacy_visualstudio_url() {
        let identity = AzureProvider
            .parse_url("https://acme.visualstudio.com/platform/_git/agents")
            .await
            .unwrap();
        assert_eq!(identity.org, "acme");
        assert_eq!(identity.project.as_deref(), Some("platform"));
    }

    #[test]
    fn pat_basic_header_uses_empty_username() {
        let header = pat_basic_header("token123");
        assert_eq!(header, format!("Basic {}", STANDARD.encode(":token123")));
    }
}
