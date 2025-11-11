use std::path::Path;

use crate::cli::AddArgs;
use crate::config::{Config, RepositoryConfig};
use crate::git::clone::{clone_repository_gix, get_clone_path};

fn args_to_config(args: &AddArgs, data_dir: &Path) -> eyre::Result<RepositoryConfig> {
    // This default path is why we can't just 'impl From<AddArgs> for RepositoryConfig'
    let path = match &args.path {
        Some(path) => path.clone(),
        None => get_clone_path(data_dir, &args.url)?,
    };

    Ok(RepositoryConfig {
        path,
        url: args.url.clone(),
        branch: args.branch.clone(),
        remote_username: args.remote_username.clone(),
        ssh_private_key: args.ssh_private_key.clone(),
        ssh_public_key: args.ssh_public_key.clone(),
        ssh_passphrase: args.ssh_passphrase.clone(),
        https_password: args.https_password.clone(),
    })
}

pub fn add(args: &AddArgs, config: &mut Config, data_dir: &Path) -> eyre::Result<()> {
    // The name exists purely to make the TOML look pretty, with the added benefit of providing a
    // unique handle for different (URL, Branch) pairs.
    let name = match &args.name {
        Some(name) => name.clone(),
        None => {
            let Some((_, name)) = args.url.rsplit_once('/') else {
                eyre::bail!(
                    "Failed to parse name from clone URL '{}'; no '/'s?",
                    args.url
                )
            };
            name.to_string()
        }
    };

    let repo_config = args_to_config(args, data_dir)?;

    if !args.skip_clone {
        let _repo = clone_repository_gix(&repo_config, args.force)?;
    }

    config.repositories.insert(name, repo_config);
    Ok(())
}
