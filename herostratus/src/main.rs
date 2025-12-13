use std::io::IsTerminal;

use clap::Parser;
use eyre::WrapErr;
use tracing_subscriber::EnvFilter;

fn main() -> eyre::Result<()> {
    let args = herostratus::cli::Args::parse();
    let use_color = std::io::stdout().is_terminal() || args.color;
    if use_color {
        color_eyre::install()?;
    }

    let proj_dir = directories::ProjectDirs::from("com", "Notgnoshi", "Herostratus").ok_or(
        eyre::eyre!("Failed to determine Herostratus data directory"),
    )?;
    let data_dir = proj_dir.data_local_dir().to_owned();
    let data_dir = args.data_dir.clone().unwrap_or(data_dir.to_path_buf());

    if args.get_data_dir {
        println!("{}", data_dir.display());
        return Ok(());
    }

    let filter = EnvFilter::builder()
        .with_default_directive(args.log_level.into())
        .with_env_var("HEROSTRATUS_LOG")
        .from_env_lossy();
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_ansi(use_color)
        .with_writer(std::io::stderr)
        .init();

    if args.get_config {
        let config = herostratus::config::read_config(&data_dir)?;
        println!("{}", herostratus::config::serialize_config(&config)?);
        return Ok(());
    }

    if args.list_rules {
        let rules = herostratus::rules::builtin_rules_all();
        let mut descriptors: Vec<_> = rules.iter().flat_map(|r| r.get_descriptors()).collect();

        descriptors.sort_by_key(|d| d.id);
        for desc in descriptors {
            println!("{:25}\t{}", desc.pretty_id(), desc.description);
        }
        return Ok(());
    }

    match args.command {
        // Shortcoming of clap; you can't have required_unless_present_any=[] for subcommands
        None => {
            eyre::bail!("Missing required subcommand");
        }
        // check is supposed to be stateless, but it's darned convenient to be able to pass in
        // RulesConfigs from the config file, especially for the purpose of integration testing! So
        // if --data-dir was passed from the CLI, we read the config, otherwise we don't.
        Some(herostratus::cli::Command::Check(cargs)) => {
            if let Some(dir) = &args.data_dir {
                tracing::warn!("Reading configs from --data-dir={dir:?}");
            }
            let config = args
                .data_dir
                .map(|d| herostratus::config::read_config(&d).unwrap());
            let stats = herostratus::commands::check(&cargs, config.as_ref()).wrap_err(format!(
                "Failed to check repository {:?} reference {:?}",
                cargs.path.display(),
                cargs.reference
            ))?;

            if cargs.summary {
                stats.print_summary();
            }
        }
        // The other subcommands are stateful, and require reading the application configuration
        Some(command) => {
            let mut config = herostratus::config::read_config(&data_dir)?;
            match command {
                herostratus::cli::Command::Add(args) => {
                    herostratus::commands::add(&args, &mut config, &data_dir)
                        .wrap_err(format!("Failed to add repository with url: {:?}", args.url))?;
                }
                herostratus::cli::Command::Remove(args) => {
                    herostratus::commands::remove(&args, &mut config, &data_dir)
                        .wrap_err(format!("Failed to remove repository: {:?}", args.url))?;
                }
                herostratus::cli::Command::CheckAll(args) => {
                    let stats = herostratus::commands::check_all(&args, &config, &data_dir)
                        .wrap_err("Failed to check all repositories")?;
                    if args.summary {
                        herostratus::commands::print_check_all_summary(&stats);
                    }
                }
                herostratus::cli::Command::FetchAll(args) => {
                    herostratus::commands::fetch_all(&args, &config, &data_dir)
                        .wrap_err("Failed to fetch all repositories")?;
                }
                _ => unreachable!(),
            }

            // Write the modified Config (in the case of Add and Remove subcommands) to the config file
            herostratus::config::write_config(&data_dir, &config)?;
        }
    }

    Ok(())
}
