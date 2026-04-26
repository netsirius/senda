//! Concrete clone / pull / push / list_branches implementations on top of
//! libgit2. They're identical for every provider — what differs is auth and
//! whether the remote also exposes a PR API. Each provider's `RepoProvider`
//! impl forwards to these helpers and adds its own auth header / token.

use std::path::{Path, PathBuf};

use git2::{
    build::RepoBuilder, BranchType, Cred, CredentialType, FetchOptions, PushOptions,
    RemoteCallbacks,
};

use crate::{Auth, Branch, ProviderError, PullResult};

/// Build a credentials callback that hands libgit2 the right [`Cred`] given
/// a Senda [`Auth`]. Most SaaS providers accept `userpass_plaintext` with the
/// PAT in either field; we use `x-access-token` / token to match GitHub
/// conventions, which Azure also accepts.
pub fn credentials_callback(
    auth: Auth,
) -> impl Fn(&str, Option<&str>, CredentialType) -> Result<Cred, git2::Error> {
    move |_url, _username_from_url, _allowed| match &auth {
        Auth::None => Cred::default(),
        Auth::PersonalAccessToken(token) => Cred::userpass_plaintext("x-access-token", token),
        Auth::DeviceFlow { access_token } => {
            Cred::userpass_plaintext("x-access-token", access_token)
        }
    }
}

pub fn clone_repo(url: &str, dest: &Path, auth: Auth) -> Result<PathBuf, ProviderError> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ProviderError::Other(e.into()))?;
    }
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(credentials_callback(auth));
    let mut fetch_opts = FetchOptions::new();
    fetch_opts.remote_callbacks(callbacks);

    RepoBuilder::new()
        .fetch_options(fetch_opts)
        .clone(url, dest)
        .map_err(map_git_err)?;
    Ok(dest.to_path_buf())
}

pub fn pull_repo(repo_path: &Path, auth: Auth) -> Result<PullResult, ProviderError> {
    let repo = git2::Repository::open(repo_path).map_err(map_git_err)?;
    let head = repo.head().ok();
    let head_oid_before = head.as_ref().and_then(|h| h.target());

    let branch_name = head
        .and_then(|h| h.shorthand().map(str::to_string))
        .unwrap_or_else(|| "main".to_string());

    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(credentials_callback(auth));
    let mut fetch_opts = FetchOptions::new();
    fetch_opts.remote_callbacks(callbacks);

    let mut remote = repo.find_remote("origin").map_err(map_git_err)?;
    remote
        .fetch(&[branch_name.as_str()], Some(&mut fetch_opts), None)
        .map_err(map_git_err)?;

    let fetch_head = repo
        .find_reference("FETCH_HEAD")
        .map_err(map_git_err)?
        .target();
    let mut updated = false;
    let mut commits_pulled = 0u32;

    if let (Some(before), Some(after)) = (head_oid_before, fetch_head) {
        if before != after {
            updated = true;
            // Count how many commits we pulled in.
            let mut walk = repo.revwalk().map_err(map_git_err)?;
            walk.push(after).map_err(map_git_err)?;
            walk.hide(before).map_err(map_git_err)?;
            commits_pulled = walk.count() as u32;

            // Fast-forward the local branch to the fetched tip.
            let analysis = repo
                .merge_analysis(&[&repo.find_annotated_commit(after).map_err(map_git_err)?])
                .map_err(map_git_err)?;
            if analysis.0.is_fast_forward() {
                let mut reference = repo
                    .find_reference(&format!("refs/heads/{branch_name}"))
                    .map_err(map_git_err)?;
                reference
                    .set_target(after, "Senda: fast-forward pull")
                    .map_err(map_git_err)?;
                repo.set_head(&format!("refs/heads/{branch_name}"))
                    .map_err(map_git_err)?;
                repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
                    .map_err(map_git_err)?;
            }
        }
    }

    Ok(PullResult {
        updated,
        commits_pulled,
    })
}

pub fn push_repo(repo_path: &Path, branch: &str, auth: Auth) -> Result<(), ProviderError> {
    let repo = git2::Repository::open(repo_path).map_err(map_git_err)?;
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(credentials_callback(auth));
    let mut push_opts = PushOptions::new();
    push_opts.remote_callbacks(callbacks);

    let mut remote = repo.find_remote("origin").map_err(map_git_err)?;
    let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");
    remote
        .push(&[refspec.as_str()], Some(&mut push_opts))
        .map_err(map_git_err)?;
    Ok(())
}

pub fn list_local_branches(repo_path: &Path) -> Result<Vec<Branch>, ProviderError> {
    let repo = git2::Repository::open(repo_path).map_err(map_git_err)?;
    let head_branch = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().map(str::to_string));
    let branches = repo
        .branches(Some(BranchType::Local))
        .map_err(map_git_err)?;
    let mut out = Vec::new();
    for b in branches {
        let (branch, _) = b.map_err(map_git_err)?;
        if let Some(name) = branch.name().map_err(map_git_err)? {
            let is_default = head_branch.as_deref() == Some(name);
            out.push(Branch {
                name: name.to_string(),
                is_default,
            });
        }
    }
    Ok(out)
}

fn map_git_err(err: git2::Error) -> ProviderError {
    ProviderError::Other(err.into())
}
