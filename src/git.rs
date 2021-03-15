use anyhow::{Context, Result};
use git2::{BranchType, Cred, CredentialType, Direction, RemoteCallbacks, Repository};
use std::env;

#[derive(Clone)]
pub struct BranchSummary {
    pub name: String,
}

/// Get the Git repo in the same dir that this binary was called from.
pub fn get_current_repo() -> Result<Repository> {
    let path = env::current_dir().context("Couldn't get the current directory")?;
    // println!("{:?}", path);
    Ok(Repository::discover(path).context("Couldn't find a git repo at the current directory")?)
}

/// Done for Git side effects
pub fn create_and_use_branch(repo: &Repository, branch_name: String) -> Result<()> {
    let default_branch = get_default_branch(repo);
    let main_branch = repo.refname_to_id(&default_branch)?;
    let main_commit = repo.find_commit(main_branch)?;
    if repo.find_branch(&branch_name, BranchType::Local).is_err() {
        let _ = repo.branch(&branch_name, &main_commit, false)?;
    }
    let refname = format!("refs/heads/{}", branch_name);
    repo.set_head(&refname)?;

    Ok(())
}

/// Try to find a default branch based on the origin, if no origin remote exists or anything else
/// happens, assume `main`.
fn get_default_branch(repo: &Repository) -> String {
    match repo.find_remote("origin") {
        Ok(mut remote) => {
            // Connect to fetch the default branch
            // Assumes an SSH key agent is available for now
            let mut callbacks = RemoteCallbacks::new();
            callbacks.credentials(git_credentials_callback);
            let _ = remote.connect_auth(Direction::Fetch, Some(callbacks), None);
            let _ = remote.disconnect();
            match remote.default_branch() {
                Ok(buf) => buf.as_str().unwrap_or("refs/heads/main").to_string(),
                Err(_) => "refs/heads/main".to_string(),
            }
        }
        Err(_) => "refs/heads/main".to_string(),
    }
}

/// Check out a branch given by a short-name. Done for Git side effects.
pub fn checkout_branch(repo: &Repository, branch_name: String) -> Result<()> {
    let refname = format!("refs/heads/{}", branch_name);
    repo.set_head(&refname)?;

    Ok(())
}

pub fn matching_branches(repo: &Repository, branch_name: String) -> Result<Vec<BranchSummary>> {
    let branches = repo.branches(Some(BranchType::Local))?;
    Ok(branches
        .filter_map(|branch| {
            if let Ok((branch, _branch_type)) = branch {
                let name = branch
                    .name()
                    .unwrap_or(None)
                    .unwrap_or("Invalid Branch")
                    .to_string();
                if name.starts_with(&branch_name) {
                    Some(BranchSummary { name })
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect())
}

pub fn git_credentials_callback(
    _user: &str,
    _user_from_url: Option<&str>,
    _cred: CredentialType,
) -> Result<Cred, git2::Error> {
    let user = _user_from_url.unwrap_or("git");

    if _cred.contains(CredentialType::USERNAME) {
        return Cred::username(user);
    }

    Cred::ssh_key_from_agent(user)
}
