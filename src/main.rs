use std::io::IsTerminal;

use clap::Parser;
use eyre::WrapErr;
use tracing_subscriber::EnvFilter;

fn main() -> eyre::Result<()> {
    let use_color = std::io::stdout().is_terminal();
    if use_color {
        color_eyre::install()?;
    }

    let args = herostratus::cli::Args::parse();
    let proj_dir = directories::ProjectDirs::from("com", "Notgnoshi", "Herostratus").ok_or(
        eyre::eyre!("Failed to determine Herostratus data directory"),
    )?;
    let data_dir = proj_dir.data_local_dir().to_owned();
    let data_dir = args.data_dir.clone().unwrap_or(data_dir.to_path_buf());

    if args.get_data_dir {
        println!("{}", data_dir.display());
        return Ok(());
    }
    if args.get_config {
        println!("{args:?}");
        return Ok(());
    }

    let filter = EnvFilter::builder()
        .with_default_directive(args.log_level.into())
        .with_env_var("HEROSTRATUS_LOG")
        .from_env_lossy();
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(use_color)
        .with_writer(std::io::stderr)
        .init();

    match args.command {
        // Shortcoming of clap; you can't have required_unless_present_any=[] for subcommands
        None => {
            eyre::bail!("Missing required subcommand");
        }
        // check is stateless. It does not touch the data dir.
        Some(herostratus::cli::Command::Check(args)) => herostratus::commands::check(&args)
            .wrap_err(format!(
                "Failed to check repository {:?} reference {:?}",
                args.path.display(),
                args.reference
            )),
        Some(herostratus::cli::Command::Add(args)) => herostratus::commands::add(&args, &data_dir)
            .wrap_err(format!("Failed to add repository with url: {:?}", args.url)),
        Some(herostratus::cli::Command::CheckAll(args)) => {
            herostratus::commands::check_all(&args, &data_dir)
                .wrap_err("Failed to check all repositories")
        }
        Some(herostratus::cli::Command::FetchAll(args)) => {
            herostratus::commands::fetch_all(&args, &data_dir)
                .wrap_err("Failed to fetch all repositories")
        }
        Some(herostratus::cli::Command::Remove(args)) => {
            herostratus::commands::remove(&args, &data_dir)
                .wrap_err(format!("Failed to remove repository: {:?}", args.url))
        }
    }
}
