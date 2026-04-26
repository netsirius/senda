use async_trait::async_trait;
use serde::Serialize;
use std::path::{Path, PathBuf};
use url::Url;

use senda_core::RepoProviderKind;

use crate::{
    git_ops, http::client, http::map_reqwest, Auth, Branch, GitHubProvider, PrInfo, PrRequest,
    ProviderError, PullResult, RepoIdentity, RepoProvider,
};

#[derive(Serialize)]
struct CreatePrBody<'a> {
    title: &'a str,
    head: &'a str,
    base: &'a str,
    body: &'a str,
}

#[derive(serde::Deserialize)]
struct PrResponse {
    html_url: String,
    number: u32,
}

#[derive(Serialize)]
struct ReviewersBody<'a> {
    reviewers: &'a [String],
}

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
        Ok(RepoIdentity {
            provider: RepoProviderKind::Github,
            org: segments[0].to_string(),
            project: None,
            repo: segments[1].trim_end_matches(".git").to_string(),
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
        // Pull without auth — for clone+pull cycles the credential helper from
        // the first clone is enough. Phase 2 keeps it simple; once we route
        // through the in-app keyring we'll surface auth here too.
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
        let token = match auth {
            Auth::PersonalAccessToken(t) | Auth::DeviceFlow { access_token: t } => t.clone(),
            Auth::None => return Err(ProviderError::Unsupported),
        };
        let endpoint = format!(
            "https://api.github.com/repos/{}/{}/pulls",
            repo.org, repo.repo
        );
        let resp: PrResponse = client()
            .post(&endpoint)
            .bearer_auth(&token)
            .header("Accept", "application/vnd.github+json")
            .json(&CreatePrBody {
                title: &pr.title,
                head: &pr.head,
                base: &pr.base,
                body: &pr.body,
            })
            .send()
            .await
            .map_err(map_reqwest)?
            .error_for_status()
            .map_err(map_reqwest)?
            .json()
            .await
            .map_err(map_reqwest)?;

        if !pr.reviewers.is_empty() {
            let reviewer_endpoint = format!(
                "https://api.github.com/repos/{}/{}/pulls/{}/requested_reviewers",
                repo.org, repo.repo, resp.number
            );
            // Best-effort — if the reviewer call fails, the PR still exists.
            let _ = client()
                .post(&reviewer_endpoint)
                .bearer_auth(&token)
                .header("Accept", "application/vnd.github+json")
                .json(&ReviewersBody {
                    reviewers: &pr.reviewers,
                })
                .send()
                .await;
        }

        Ok(PrInfo {
            url: resp.html_url,
            number: resp.number,
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
    async fn parses_well_formed_github_url() {
        let identity = GitHubProvider
            .parse_url("https://github.com/freenet/freenet-core")
            .await
            .unwrap();
        assert_eq!(identity.org, "freenet");
        assert_eq!(identity.repo, "freenet-core");
    }

    #[tokio::test]
    async fn strips_dot_git_suffix() {
        let identity = GitHubProvider
            .parse_url("https://github.com/foo/bar.git")
            .await
            .unwrap();
        assert_eq!(identity.repo, "bar");
    }

    #[tokio::test]
    async fn rejects_url_without_repo() {
        let err = GitHubProvider.parse_url("https://github.com/foo").await;
        assert!(matches!(err, Err(ProviderError::InvalidUrl(_))));
    }
}
