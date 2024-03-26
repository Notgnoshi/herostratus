use std::path::Path;

use git2::Repository;

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
