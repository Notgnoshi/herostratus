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

fn clone_credentials(
    config: &crate::config::RepositoryConfig,
    parsed_username: Option<&str>,
) -> eyre::Result<git2::Cred> {
    let username = config
        .remote_username
        .as_deref()
        .unwrap_or(parsed_username.unwrap_or("git"));

    if config.remote_url.starts_with("https://") {
        if let Some(password) = &config.https_password {
            Ok(git2::Cred::userpass_plaintext(username, password)?)
        } else {
            let git_config = git2::Config::open_default()?;
            Ok(git2::Cred::credential_helper(
                &git_config,
                &config.remote_url,
                Some(username),
            )?)
        }
    } else if config.remote_url.starts_with("ssh://") || config.remote_url.contains('@') {
        if let Some(priv_key) = &config.ssh_private_key {
            Ok(git2::Cred::ssh_key(
                username,
                config.ssh_public_key.as_deref(),
                priv_key,
                config.ssh_passphrase.as_deref(),
            )?)
        } else {
            Ok(git2::Cred::ssh_key_from_agent(username)?)
        }
    } else {
        Ok(git2::Cred::default()?)
    }
}

pub fn clone_repository(
    config: &crate::config::RepositoryConfig,
    force: bool,
) -> eyre::Result<git2::Repository> {
    let start = std::time::Instant::now();
    tracing::info!(
        "Cloning {:?} to {} ...",
        config.remote_url,
        config.path.display()
    );

    if config.path.exists() {
        tracing::warn!("{} already exists", config.path.display());
        if force {
            tracing::info!("Deleting {} ...", config.path.display());
            remove_dir_contents(&config.path).wrap_err(format!(
                "Failed to force clone {:?} to {}",
                config.remote_url,
                config.path.display()
            ))?;
        } else {
            eyre::bail!("{} already exists", config.path.display());
        }
    }

    let mut builder = git2::build::RepoBuilder::new();
    builder.bare(true);

    let mut callbacks = git2::RemoteCallbacks::new();
    callbacks.credentials(|_url, username_from_url, _allowed_typed| {
        match clone_credentials(config, username_from_url) {
            Ok(creds) => Ok(creds),
            Err(e) => Err(git2::Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Config,
                format!("Failed to determine appropriate clone credentials: {e:?}"),
            )),
        }
    });
    let mut options = git2::FetchOptions::new();
    options.remote_callbacks(callbacks);
    builder.fetch_options(options);

    if let Some(branch) = &config.branch {
        tracing::debug!("Cloning just the '{branch}' branch ...");
        builder.branch(branch);
    }

    let repo = builder
        .clone(&config.remote_url, &config.path)
        .wrap_err("Failed to clone repository")?;
    tracing::info!(
        "Finished cloning {:?} after {:?}",
        config.remote_url,
        start.elapsed()
    );
    Ok(repo)
}
