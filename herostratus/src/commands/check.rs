use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::time::{Duration, Instant};

use crate::achievement::{AchievementEvent, grant};
use crate::cli::{CheckAllArgs, CheckArgs, CheckOneArgs};
use crate::commands::fetch_all::{FetchStat, fetch_one};
use crate::config::Config;
use crate::git::clone::find_local_repository;

#[derive(Clone, Debug, Default)]
pub struct CheckStat {
    pub name: String,
    pub num_commits_checked: u64,
    pub num_achievements_granted: u64,
    pub elapsed: Duration,
    /// Per-achievement grant counts, sorted by descriptor ID. Each entry is (pretty_id, count).
    pub counts: Vec<(String, u64)>,
}

impl CheckStat {
    pub fn print_summary(&self) {
        let time_per_commit = if self.num_commits_checked == 0 {
            Duration::ZERO
        } else {
            self.elapsed / self.num_commits_checked as u32
        };
        // Marker to distinguish between achievements and summary, both on stdout
        println!("## Summary");
        println!("| Name | # Commits | # Achievements | Time | Time per commit |");
        println!("| ---- | --------- | -------------- | ---- | --------------- |");
        println!(
            "| {} | {} | {} | {:.2?} | {:.2?} |",
            self.name,
            self.num_commits_checked,
            self.num_achievements_granted,
            self.elapsed,
            time_per_commit
        );
    }
}

// Stateless; do not allow filesystem modification, or reading from application data (unless
// --data-dir was *explicitly* passed)
pub fn check(args: &CheckArgs, config: Option<&Config>) -> eyre::Result<CheckStat> {
    let name = args
        .path
        .file_name()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or("".into());

    check_impl(
        config,
        &name,
        &args.path,
        &args.reference,
        args.depth,
        None,
        None,
    )
}

fn check_impl(
    config: Option<&Config>,
    name: &str,
    path: &Path,
    reference: &str,
    depth: Option<usize>,
    data_dir: Option<&Path>,
    repo_config: Option<&crate::config::RepositoryConfig>,
) -> eyre::Result<CheckStat> {
    tracing::info!("Checking repository {path:?}, reference {reference:?} for achievements ...");
    let mut repo = find_local_repository(path)?;
    let mut events = Vec::new();
    let stats = grant(
        config,
        reference,
        &mut repo,
        depth,
        data_dir,
        name,
        repo_config,
        |e| {
            process_event(&e);
            events.push(e);
        },
    )?;

    if let Some(data_dir) = data_dir
        && stats.num_commits_processed > 0
    {
        let repo_config = config.and_then(|c| c.repositories.get(name));
        let url = repo_config.map(|rc| rc.url.as_str()).unwrap_or("");
        let commit_url_prefix = repo_config.and_then(|rc| rc.resolve_commit_url_prefix());
        crate::achievement::upsert_repository_csv(
            data_dir,
            name,
            url,
            commit_url_prefix.as_deref(),
            reference,
            stats.num_commits_processed,
        )?;
    }

    let counts = tally_achievements(&events);
    for (pretty_id, count) in &counts {
        tracing::info!("{pretty_id}: {count}");
    }

    Ok(CheckStat {
        name: name.to_string(),
        num_commits_checked: stats.num_commits_processed,
        num_achievements_granted: stats.num_achievements_generated,
        elapsed: stats.elapsed,
        counts,
    })
}

#[derive(Clone, Debug)]
pub struct CheckAllStat {
    pub name: String,
    pub num_commits_fetched: Option<u64>,
    pub fetch_duration: Option<Duration>,
    pub num_commits_checked: u64,
    pub num_achievements_granted: u64,
    pub check_duration: Duration,
    /// Per-achievement grant counts, sorted by descriptor ID. Each entry is (name, count).
    pub counts: Vec<(String, u64)>,
}

pub fn print_check_all_summary(stats: &[CheckAllStat]) {
    println!("## Summary");
    println!("| Name | # Commits | # Achievements | Time | Time per commit |");
    println!("| ---- | --------- | -------------- | ---- | --------------- |");
    for stat in stats {
        let time_per_commit = if stat.num_commits_checked == 0 {
            Duration::ZERO
        } else {
            stat.check_duration / stat.num_commits_checked as u32
        };
        println!(
            "| {} | {} | {} | {:.2?} | {:.2?} |",
            stat.name,
            stat.num_commits_checked,
            stat.num_achievements_granted,
            stat.check_duration,
            time_per_commit
        );
    }
}

fn merge_stats(fetch: Vec<FetchStat>, check: Vec<CheckStat>) -> Vec<CheckAllStat> {
    let fetch: HashMap<String, FetchStat> =
        HashMap::from_iter(fetch.into_iter().map(|f| (f.name.clone(), f)));
    let check: HashMap<String, CheckStat> =
        HashMap::from_iter(check.into_iter().map(|c| (c.name.clone(), c)));

    let mut merged = Vec::new();
    for (name, check_stat) in check {
        let fetch_stat = fetch.get(&name);
        let stat = CheckAllStat {
            name,
            num_commits_fetched: fetch_stat.and_then(|f| f.num_commits_fetched),
            fetch_duration: fetch_stat.map(|f| f.elapsed),
            num_commits_checked: check_stat.num_commits_checked,
            num_achievements_granted: check_stat.num_achievements_granted,
            check_duration: check_stat.elapsed,
            counts: check_stat.counts,
        };
        merged.push(stat);
    }

    merged
}

pub fn check_all(
    args: &CheckAllArgs,
    config: &Config,
    data_dir: &Path,
) -> eyre::Result<Vec<CheckAllStat>> {
    let mut fetch_stats = Vec::new();
    let mut check_stats = Vec::new();
    if !args.no_fetch {
        fetch_stats = crate::commands::fetch_all(&args.into(), config, data_dir)?;
    }

    tracing::info!("Checking repositories ...");
    let start = Instant::now();
    for (name, repo_config) in config.repositories.iter() {
        let reference = repo_config
            .reference
            .clone()
            .unwrap_or_else(|| String::from("HEAD"));
        let check_stat = check_impl(
            Some(config),
            name,
            &repo_config.path,
            &reference,
            args.depth,
            Some(data_dir),
            Some(repo_config),
        )?;
        check_stats.push(check_stat);
    }
    tracing::info!(
        "... checked {} repositories after {:.2?}",
        config.repositories.len(),
        start.elapsed()
    );

    Ok(merge_stats(fetch_stats, check_stats))
}

/// Look up a repository by name or remote URL
fn find_repository<'a>(
    config: &'a Config,
    repository: &str,
) -> eyre::Result<(&'a str, &'a crate::config::RepositoryConfig)> {
    // Try name match first
    if let Some((name, repo_config)) = config.repositories.get_key_value(repository) {
        return Ok((name, repo_config));
    }

    // Try URL match
    for (name, repo_config) in config.repositories.iter() {
        if repo_config.url == repository {
            return Ok((name, repo_config));
        }
    }

    let available: Vec<_> = config.repositories.keys().collect();
    eyre::bail!(
        "Repository {:?} not found. Available repositories: {:?}",
        repository,
        available
    );
}

pub fn check_one(
    args: &CheckOneArgs,
    config: &Config,
    data_dir: &Path,
) -> eyre::Result<Vec<CheckAllStat>> {
    let (name, repo_config) = find_repository(config, &args.repository)?;

    let mut fetch_stats = Vec::new();
    if !args.no_fetch {
        fetch_stats.push(fetch_one(name, repo_config, data_dir)?);
    }

    let reference = repo_config
        .reference
        .clone()
        .unwrap_or_else(|| String::from("HEAD"));
    let check_stat = check_impl(
        Some(config),
        name,
        &repo_config.path,
        &reference,
        args.depth,
        Some(data_dir),
        Some(repo_config),
    )?;

    Ok(merge_stats(fetch_stats, vec![check_stat]))
}

/// A common event sink that both check and check_all can use
fn process_event(event: &AchievementEvent) {
    // TODO: Support different output formats
    println!("{event:?}");
}

/// Count grants per achievement, sorted by descriptor ID.
fn tally_achievements(events: &[AchievementEvent]) -> Vec<(String, u64)> {
    let mut counts: BTreeMap<usize, (String, u64)> = BTreeMap::new();
    for event in events {
        if let AchievementEvent::Grant(a) = event {
            let pretty_id = format!("H{}-{}", a.descriptor_id, a.human_id);
            counts
                .entry(a.descriptor_id)
                .and_modify(|(_, c)| *c += 1)
                .or_insert((pretty_id, 1));
        }
    }
    counts.into_values().collect()
}
