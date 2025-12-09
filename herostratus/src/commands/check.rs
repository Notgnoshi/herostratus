use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::achievement::{Achievement, grant};
use crate::cli::{CheckAllArgs, CheckArgs};
use crate::commands::fetch_all::FetchStat;
use crate::config::Config;
use crate::git::clone::find_local_repository;

#[derive(Clone, Debug, Default)]
pub struct CheckStat {
    pub name: String,
    pub num_commits_checked: u64,
    pub num_achievements_granted: u64,
    pub elapsed: Duration,
}

impl CheckStat {
    pub fn print_summary(&self) {
        let time_per_commit = self.elapsed / self.num_commits_checked as u32;
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
    check_impl(config, &name, &args.path, &args.reference, args.depth)
}

fn check_impl(
    config: Option<&Config>,
    name: &str,
    path: &Path,
    reference: &str,
    depth: Option<usize>,
) -> eyre::Result<CheckStat> {
    tracing::info!("Checking repository {path:?}, reference {reference:?} for achievements ...");
    let mut stat = CheckStat {
        name: name.to_string(),
        ..Default::default()
    };
    let start = Instant::now();
    let repo = find_local_repository(path)?;
    let mut achievements = grant(config, reference, &repo, depth)?;

    process_achievements(&mut achievements)?;

    stat.num_commits_checked = achievements.num_commits_processed;
    stat.num_achievements_granted = achievements.num_achievements_generated;
    stat.elapsed = start.elapsed();

    Ok(stat)
}

#[derive(Clone, Debug)]
pub struct CheckAllStat {
    pub name: String,
    pub num_commits_fetched: Option<u64>,
    pub fetch_duration: Option<Duration>,
    pub num_commits_checked: u64,
    pub num_achievements_granted: u64,
    pub check_duration: Duration,
}

pub fn print_check_all_summary(stats: &[CheckAllStat]) {
    println!("## Summary");
    println!("| Name | # Commits | # Achievements | Time | Time per commit |");
    println!("| ---- | --------- | -------------- | ---- | --------------- |");
    for stat in stats {
        let time_per_commit = stat.check_duration / stat.num_commits_checked as u32;
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

/// A common achievement sink that both check and check_all can use
fn process_achievements(achievements: impl Iterator<Item = Achievement>) -> eyre::Result<()> {
    // TODO: Support different output formats
    for achievement in achievements {
        println!("{achievement:?}");
    }
    Ok(())
}
