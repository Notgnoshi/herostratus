use std::path::{Path, PathBuf};

use eyre::WrapErr;

pub enum RepoType {
    LocalFilePath(PathBuf),
    RemoteCloneUrl(String),
}

/// Parse the clone URL and determine if it's a local file path or remote
// TODO(#26): Support ~username expansion for URLs like:
//      ssh://[user@]host.xz[:port]/~[user]/path/to/repo.git/
//      git://host.xz[:port]/~[user]/path/to/repo.git/
//      [user@]host.xz:/~[user]/path/to/repo.git/
// This is currently left out-of-scope for simplicity of implementation, and because I don't work
// with repositories where this is used.
pub fn local_or_remote(repo: &str) -> eyre::Result<RepoType> {
    let known_remote_protocols = [
        "ssh://", "git://", "http://", "https://", "ftp://", "ftps://",
    ];
    for proto in known_remote_protocols {
        if repo.starts_with(proto) {
            return Ok(RepoType::RemoteCloneUrl(repo.to_string()));
        }
    }
    // It could also be a valid SSH URL if it contains a colon:
    //      [user@]example.com:path/to/repo.git
    // but only if there are no slashes before the colon.
    if let Some(colon_index) = repo.find(':') {
        if let Some(slash_index) = repo.find('/') {
            if slash_index < colon_index {
                let path = PathBuf::from(repo).canonicalize()?;
                return Ok(RepoType::LocalFilePath(path));
            }
        }
        return Ok(RepoType::RemoteCloneUrl(repo.to_string()));
    }

    if let Some(repo) = repo.strip_prefix("file://") {
        let path = PathBuf::from(repo).canonicalize()?;
        return Ok(RepoType::LocalFilePath(path));
    }

    let path = PathBuf::from(repo).canonicalize()?;
    Ok(RepoType::LocalFilePath(path))
}

pub fn find_local_repository(path: &Path) -> eyre::Result<git2::Repository> {
    tracing::info!("Searching local path {path:?} for a Git repository");
    let repo = git2::Repository::discover(path)?;
    tracing::info!("Found local git repository at {:?}", repo.path());
    Ok(repo)
}

// ssh://git@example.com/path.git           => path.git
// git@github.com:Notgnoshi/herostratus.git => Notgnoshi/herostratus.git
// https://example.com/foo                  => foo
// domain:path                              => path
pub fn parse_path_from_url(url: &str) -> eyre::Result<PathBuf> {
    let known_remote_protocols = [
        "ssh://", "git://", "http://", "https://", "ftp://", "ftps://", "file://",
    ];
    for protocol in known_remote_protocols {
        if let Some(url) = url.strip_prefix(protocol) {
            let Some(idx) = url.find('/') else {
                eyre::bail!("Failed to find '/' separator in {url:?}");
            };
            return Ok(PathBuf::from(&url[idx + 1..]));
        }
    }

    if url.contains("://") {
        eyre::bail!("Found unsupported protocol from URL {url:?}");
    }

    // If the URL doesn't begin with a protocol, it's likely the alternative SSH syntax, where the
    // path begins after the ':'
    //
    // TODO(#26): Support ~username expansion.

    let Some(idx) = url.find(':') else {
        eyre::bail!("Failed to find ':' separator in {url:?}")
    };
    Ok(PathBuf::from(&url[idx + 1..]))
}

fn remove_dir_contents<P: AsRef<Path>>(path: P) -> std::io::Result<()> {
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();

        if entry.file_type()?.is_dir() {
            remove_dir_contents(&path)?;
            std::fs::remove_dir(path)?;
        } else {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

/// Clone a remote URL, or fetch from a previous clone
///
/// Projects will be cloned to a platform-dependent project data dir. See
/// <https://docs.rs/directories/latest/directories/struct.ProjectDirs.html#method.data_local_dir>
/// for details.
///
/// If the given URL has already been cloned, fetch from the remote instead.
pub fn clone_or_cache_remote_repository(
    url: &str,
    data_dir: &Path,
    force_clone: bool,
    skip_fetch: bool,
) -> eyre::Result<git2::Repository> {
    let start = std::time::Instant::now();

    let clone_path = parse_path_from_url(url).wrap_err("Failed to parse clone path from URL")?;
    let clone_path = data_dir.join("git").join(clone_path);

    tracing::info!(
        "Attempting to clone remote URL {url:?} to {}",
        clone_path.display()
    );

    if !clone_path.exists() || force_clone {
        tracing::debug!("Cloning {url:?}...");

        if clone_path.exists() {
            tracing::info!("Deleting existing cached clone for {url:?}");
            remove_dir_contents(&clone_path)
                .wrap_err(format!("Failed to clear cached clone for {url:?}"))?;
        }

        let repo = git2::build::RepoBuilder::new()
            .bare(true)
            // TODO(#21,#22): SSH and HTTPS auth
            // .fetch_options()
            // TODO(#33): Similar to fetch below, I _think_ this will be more efficient if it only
            // has to clone the reference provided by the user.
            // .branch(branch)
            .clone(url, &clone_path)
            .wrap_err("Failed to clone repository")?;
        tracing::info!("Finished cloning {url:?} after {:?}", start.elapsed());

        Ok(repo)
    } else {
        tracing::debug!("Found existing {}", clone_path.display());
        let repo = git2::Repository::discover(clone_path).wrap_err("Failed to use cached clone")?;

        if skip_fetch {
            tracing::debug!("Skipping fetch from {url:?}");
        } else {
            tracing::debug!("Fetching from 'origin' remote ...");
            let mut remote = repo
                .find_remote("origin")
                .wrap_err("Failed to find default 'origin' remote")?;

            // TODO(#33): This would be more efficient if it only had to fetch the reference provided
            // by the user. But the user could also provide a revision, so we'd have to determine
            // whether it's a ref or rev.
            let refspecs = remote.fetch_refspecs()?;
            let refspecs: Vec<&str> = refspecs.iter().flatten().collect();

            remote.fetch(
                refspecs.as_slice(),
                None, // TODO(#21,#22): SSH and HTTPS auth
                Some("Automated fetch by herostratus"),
            )?;

            drop(remote);
            tracing::info!("Finished fetching {url:?} after {:?}", start.elapsed());
        }

        Ok(repo)
    }
}
