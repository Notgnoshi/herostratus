mod aggregate;
mod data;
mod users;

use std::path::Path;

use eyre::WrapErr;

use crate::cli::RenderArgs;

/// Relative path prefix from root-level pages back to the site root.
const ROOT_PAGE: &str = "./";
/// Relative path prefix from pages one directory deep back to the site root.
const NESTED_PAGE: &str = "../";

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
    let users = users::derive_users(&events);

    let total_events: usize = events.values().map(|v| v.len()).sum();
    tracing::info!(
        achievements = achievements.len(),
        repositories = repositories.len(),
        event_files = events.len(),
        users = users.len(),
        total_events,
        "Loaded export data"
    );

    let site = aggregate::aggregate(&achievements, &repositories, &events, &users);
    tracing::info!(
        achievement_pages = site.achievements.len(),
        repo_pages = site.repositories.len(),
        user_pages = site.users.len(),
        recent_activity = site.recent_activity.len(),
        "Aggregated site data"
    );

    let env = load_templates(&args.templates)?;

    // Ensure output directories exist
    std::fs::create_dir_all(&args.output_dir)?;

    // Render index.html
    render_page(
        &env,
        "index.html",
        minijinja::context! {
            site_title => &args.site_title,
            root => ROOT_PAGE,
            repositories => &site.repositories,
            recent_activity => &site.recent_activity,
        },
        &args.output_dir.join("index.html"),
    )?;

    // Render achievements.html
    render_page(
        &env,
        "achievements.html",
        minijinja::context! {
            site_title => &args.site_title,
            root => ROOT_PAGE,
            achievements => &site.achievements,
        },
        &args.output_dir.join("achievements.html"),
    )?;

    // Render achievement detail pages
    for achievement in &site.achievements {
        render_page(
            &env,
            "achievement_detail.html",
            minijinja::context! {
                site_title => &args.site_title,
                root => NESTED_PAGE,
                achievement => achievement,
            },
            &args
                .output_dir
                .join(format!("achievement/{}.html", achievement.human_id)),
        )?;
    }

    tracing::info!(
        output_dir = %args.output_dir.display(),
        "Site rendered"
    );
    Ok(())
}

fn load_templates(templates_dir: &Path) -> eyre::Result<minijinja::Environment<'static>> {
    let mut env = minijinja::Environment::new();
    let dir = templates_dir.to_path_buf();
    env.set_loader(move |name| {
        let path = dir.join(name);
        match std::fs::read_to_string(&path) {
            Ok(content) => Ok(Some(content)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(minijinja::Error::new(
                minijinja::ErrorKind::InvalidOperation,
                format!("failed to read template {path:?}: {e}"),
            )),
        }
    });
    tracing::debug!("Loaded templates from {templates_dir:?}");
    Ok(env)
}

fn render_page(
    env: &minijinja::Environment<'_>,
    template_name: &str,
    context: minijinja::Value,
    output_path: &Path,
) -> eyre::Result<()> {
    let template = env
        .get_template(template_name)
        .wrap_err_with(|| format!("Failed to load template {template_name:?}"))?;
    let rendered = template
        .render(context)
        .wrap_err_with(|| format!("Failed to render template {template_name:?}"))?;

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output_path, rendered)?;
    tracing::debug!("Wrote {output_path:?}");
    Ok(())
}
