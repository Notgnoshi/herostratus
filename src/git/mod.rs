use std::path::Path;

use eyre::WrapErr;
use git2::{ObjectType, Oid, Repository};

/// Fetch or find the specified repository
///
/// If the `repo` is an accessible path, it will not be cloned. Otherwise, the `repo` is assumed to
/// be a clone URL supporting the following protocols
/// * `file://`
/// * `ssh://` or `git@`
/// * `git://`
/// * `https://`
pub fn fetch_or_find(repo: &str) -> eyre::Result<Repository> {
    // TODO: The reliability, flexibility, and clarity of this method will be very important.
    // TODO: Special case file://
    let path = Path::new(repo);
    if !path.exists() {
        // TODO: Support remote URLs
        // TODO: Test cases!
        eyre::bail!("{path:?} does not exist, and remote clone URLs are not supported yet");
    } else {
        let path = path.canonicalize()?;
        tracing::debug!("Searching {path:?} for a Git repository");
        let repo = Repository::discover(&path)?;
        tracing::info!("Found git repository at {:?}", repo.path());
        Ok(repo)
    }
}

pub fn rev_parse(reference: &str, repo: &Repository) -> eyre::Result<Oid> {
    let object = repo
        .revparse_single(reference)
        .wrap_err("Failed to rev-parse")?;
    let oid = object.id();
    tracing::info!(
        "Resolved {reference:?} to {:?} {oid:?}",
        object.kind().unwrap_or(ObjectType::Any)
    );
    Ok(oid)
}
