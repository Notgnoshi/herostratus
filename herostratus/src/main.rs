use std::io::IsTerminal;

use clap::Parser;
use eyre::WrapErr;

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

    let _trace_guard = herostratus::trace::init(args.log_level, use_color);

    if args.get_config {
        let config = herostratus::config::read_config(&data_dir)?;
        println!("{}", herostratus::config::serialize_config(&config)?);
        return Ok(());
    }

    if args.list_rules {
        let rules = herostratus::rules::builtin_rules_all();
        let mut metas: Vec<_> = rules.iter().map(|r| r.meta().clone()).collect();
        metas.extend(herostratus::achievement::meta_achievement_metas());

        metas.sort_by_key(|m| m.id);
        for meta in &metas {
            println!(
                "{:25}\t{}",
                format!("H{}-{}", meta.id, meta.human_id),
                meta.description
            );
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
                .map(|d| herostratus::config::read_config(&d))
                .transpose()?;
            let stats = herostratus::commands::check(&cargs, config.as_ref()).wrap_err(format!(
                "Failed to check repository {:?} reference {:?}",
                cargs.path.display(),
                cargs.reference
            ))?;

            if cargs.summary {
                stats.print_summary();
            }
        }
        Some(herostratus::cli::Command::Render(mut rargs)) => {
            if rargs.export_dir.is_none() && data_dir.exists() {
                rargs.export_dir = Some(data_dir.join("export"));
            } else if rargs.export_dir.is_none() {
                panic!("render subcommand requires --export-dir");
            }
            herostratus::commands::render(&rargs)?;
        }
        // The other subcommands are stateful, and require reading the application configuration
        Some(command) => {
            let mut config = herostratus::config::read_config(&data_dir)?;
            match command {
                herostratus::cli::Command::Add(args) => {
                    herostratus::commands::add(&args, &mut config, &data_dir)
                        .wrap_err(format!("Failed to add repository with url: {:?}", args.url))?;
                }
                herostratus::cli::Command::CheckOne(args) => {
                    let stats = herostratus::commands::check_one(&args, &config, &data_dir)
                        .wrap_err(format!("Failed to check repository {:?}", args.repository))?;
                    if args.summary {
                        herostratus::commands::print_check_all_summary(&stats);
                    }
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
