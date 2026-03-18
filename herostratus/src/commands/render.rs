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
    Ok(())
}
