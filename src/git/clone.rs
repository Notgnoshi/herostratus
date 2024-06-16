use std::path::{Path, PathBuf};

use eyre::WrapErr;

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

pub fn get_clone_path(data_dir: &Path, url: &str) -> eyre::Result<PathBuf> {
    let clone_path = parse_path_from_url(url).wrap_err("Failed to parse clone path from URL")?;
    let clone_path = data_dir.join("git").join(clone_path);
    Ok(clone_path)
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

pub fn clone_repository(
    path: &Path,
    url: &str,
    branch: Option<&str>,
    force: bool,
) -> eyre::Result<git2::Repository> {
    let start = std::time::Instant::now();
    tracing::info!("Cloning {url:?} to {} ...", path.display());

    if path.exists() {
        tracing::warn!("{} already exists", path.display());
        if force {
            tracing::info!("Deleting {} ...", path.display());
            remove_dir_contents(path).wrap_err(format!(
                "Failed to force clone {url:?} to {}",
                path.display()
            ))?;
        } else {
            eyre::bail!("{} already exists", path.display());
        }
    }

    let mut builder = git2::build::RepoBuilder::new();
    builder.bare(true);

    // TODO(#21,#22): SSH and HTTPS auth
    // .fetch_options()

    if let Some(branch) = branch {
        tracing::debug!("Cloning just the '{branch}' branch ...");
        builder.branch(branch);
    }

    let repo = builder
        .clone(url, path)
        .wrap_err("Failed to clone repository")?;
    tracing::info!("Finished cloning {url:?} after {:?}", start.elapsed());
    Ok(repo)
}
