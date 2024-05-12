pub mod clone;
#[cfg(test)]
mod test_clone;

use std::path::Path;

use eyre::WrapErr;
use git2::{ObjectType, Oid, Repository, Sort};

/// Fetch or find the specified repository
///
/// If the `repo` is an accessible path, it will not be cloned. Otherwise, the `repo` is assumed to
/// be a remote URL.
///
/// The [`git-clone(1)`](https://git-scm.com/docs/git-clone/) man page specifies the supported URL
/// formats. As a summary,
///
/// * SSH protocol: `ssh://[user@]example.com[:port]/path/to/repo.git`
///   * Variant: `[user@]example.com:path/to/repo.git`
/// * Git protocol: `git://example.com[:port]/path/to/repo.git`
/// * HTTP protocol: `http[s]://example.com[:port]/path/to/repo.git`
/// * FTP protocol (old and slow, do not use): `ftp[s]://example.com[:port]/path/to/repo.git`
///
/// A local path may also be used
///
/// * `./relative/path/to/repo`
/// * `/absolute/path/to/repo`
/// * `file:///absolute/path/to/repo`
///
/// The local repository paths may be to the repository work tree, the repository `.git/`
/// directory, or the path to a bare repository.
///
/// If a remote URL is passed, the repository will be cloned as a bare repository. Otherwise, if a
/// local path is passed, the existing repository will be used, with no bare repository created.
pub fn fetch_or_find(
    repo: &str,
    data_dir: &Path,
    force_clone: bool,
    skip_fetch: bool,
) -> eyre::Result<git2::Repository> {
    tracing::info!("Finding repository '{repo}' ...");
    match clone::local_or_remote(repo).wrap_err(format!("Failed to find repository: '{repo}'"))? {
        clone::RepoType::LocalFilePath(path) => clone::find_local_repository(&path),
        clone::RepoType::RemoteCloneUrl(url) => {
            clone::clone_or_cache_remote_repository(&url, data_dir, force_clone, skip_fetch)
        }
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

pub fn rev_walk(
    oid: Oid,
    repo: &Repository,
) -> eyre::Result<impl Iterator<Item = eyre::Result<Oid>> + '_> {
    let mut revwalk = repo.revwalk().wrap_err("Could not walk repository")?;
    revwalk.set_sorting(Sort::TIME | Sort::TOPOLOGICAL)?;
    revwalk.push(oid)?;

    Ok(revwalk.map(|r| r.wrap_err("Failed to yield next rev")))
}
