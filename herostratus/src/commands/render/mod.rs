mod data;

use crate::cli::RenderArgs;

pub fn render(args: &RenderArgs) -> eyre::Result<()> {
    tracing::info!(
        export_dir = %args.export_dir.display(),
        output_dir = %args.output_dir.display(),
        base_url = %args.base_url,
        site_title = %args.site_title,
        templates = %args.templates.display(),
        "Rendering static site"
    );

    let achievements = data::load_achievements(&args.export_dir)?;
    let repositories = data::load_repositories(&args.export_dir)?;
    let events = data::load_events(&args.export_dir)?;

    let total_events: usize = events.values().map(|v| v.len()).sum();
    tracing::info!(
        achievements = achievements.len(),
        repositories = repositories.len(),
        event_files = events.len(),
        total_events,
        "Loaded export data"
    );

    Ok(())
}
