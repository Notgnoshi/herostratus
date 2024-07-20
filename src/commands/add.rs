use std::path::Path;

use crate::cli::AddArgs;
use crate::config::{Config, RepositoryConfig};
use crate::git::clone::{clone_repository, get_clone_path};

pub fn add(args: &AddArgs, config: &mut Config, data_dir: &Path) -> eyre::Result<()> {
    // TODO: What *should* the name be? The whole URL? Just the path part of the URL?
    // TODO: What to do if the repository is already added?
    let Some((_, name)) = args.url.rsplit_once('/') else {
        eyre::bail!("Failed to parse URL '{}'", args.url)
    };
    let name = name.to_string();

    let clone_path = if let Some(cli_path) = &args.path {
        cli_path.to_path_buf()
    } else {
        get_clone_path(data_dir, &args.url)?
    };

    let repo_config = RepositoryConfig {
        path: clone_path,
        remote_url: args.url.clone(),
        branch: args.branch.clone(),
        ..Default::default()
    };

    let _repo = clone_repository(
        &repo_config.path,
        &args.url,
        args.branch.as_deref(),
        args.force,
    )?;
    config.repositories.insert(name, repo_config);
    Ok(())
}
