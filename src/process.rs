use console::style;
use indicatif::{HumanDuration, ProgressBar, ProgressStyle};
use std::{
    collections::HashMap,
    path::Path,
    time::{Duration, Instant},
};

use crate::{
    cli::{Args, Metrics, SortKey},
    error::AppError,
    git::{git_blame, git_list_files, git_log, git_shortlog},
    stats::AuthorStats,
    utils::sort_value,
};

pub fn process_repo(
    dir: &Path,
    args: &Args,
) -> Result<(String, Vec<(String, AuthorStats)>), AppError> {
    let repo_name = dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap()
        .to_owned();

    let started = Instant::now();

    let mut result: HashMap<String, AuthorStats> = HashMap::new();

    let files = git_list_files(dir, args)?;

    let total_steps = (args.metrics.contains(&Metrics::Files)
        || args.metrics.contains(&Metrics::History)) as u64
        + args.metrics.contains(&Metrics::Commits) as u64
        + args
            .metrics
            .contains(&Metrics::Current)
            .then(|| files.len() as u64)
            .unwrap_or(0);

    /* Bar 1: repo-level progress (known total) */
    let repo_pb = if args.silent_progress {
        ProgressBar::hidden()
    } else {
        let pb = ProgressBar::new(total_steps);
        pb.enable_steady_tick(Duration::from_millis(200));
        pb.set_style(
            ProgressStyle::with_template(
                    "{spinner:.white} {msg:.dim} [{elapsed_precise:.cyan} < {eta_precise:.dim}] ({pos:.green}/{len:.green})"
                )
                .unwrap()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
        );
        pb.set_message(format!("Processing {}...", repo_name));
        pb
    };

    let git_log_data = args
        .metrics
        .iter()
        .any(|m| matches!(m, Metrics::Files | Metrics::History))
        .then(|| git_log(dir, args, &files, &repo_pb))
        .transpose()?;

    for m in &args.metrics {
        match m {
            Metrics::Commits => {
                let commit_counts = git_shortlog(dir, args, &repo_pb)?;
                for (author, commits) in commit_counts {
                    let entry = result
                        .entry(author)
                        .or_insert_with(|| AuthorStats::for_metrics(&args.metrics));
                    entry.commits = Some(entry.commits.unwrap_or(0) + commits);
                }
            }
            Metrics::Files => {
                let history_stats = git_log_data.as_ref().unwrap();
                for (author, h) in history_stats {
                    let entry = result
                        .entry(author.clone())
                        .or_insert_with(|| AuthorStats::for_metrics(&args.metrics));
                    entry.files = Some(entry.files.unwrap_or(0) + h.files.len());
                }
            }
            Metrics::History => {
                let history_stats = git_log_data.as_ref().unwrap();
                for (author, h) in history_stats {
                    let entry = result
                        .entry(author.clone())
                        .or_insert_with(|| AuthorStats::for_metrics(&args.metrics));
                    entry.ins = Some(entry.ins.unwrap_or(0) + h.ins);
                    entry.del = Some(entry.del.unwrap_or(0) + h.del);
                    entry.net = Some(entry.net.unwrap_or(0) + h.net);
                    entry.churn = Some(entry.churn.unwrap_or(0) + h.churn);
                }
            }
            Metrics::Current => {
                let current_stats = git_blame(dir, args, &files, &repo_pb)?;
                for (author, c) in current_stats {
                    let entry = result
                        .entry(author)
                        .or_insert_with(|| AuthorStats::for_metrics(&args.metrics));
                    entry.surviving = Some(entry.surviving.unwrap_or(0) + c.surviving);
                }
            }
            Metrics::All => {}
        }
    }

    if !args.silent_progress {
        repo_pb.println(
            format!(
                "{} {} {}",
                style("Done").dim(),
                style(&repo_name).bold().dim(),
                style(format!("in {}", HumanDuration(started.elapsed()))).dim()
            )
            .to_owned(),
        );
    }

    repo_pb.finish_and_clear();

    let sort_key: SortKey = args.sort.unwrap_or_else(|| SortKey::Commits);
    let mut rows: Vec<(String, AuthorStats)> = result.into_iter().collect();

    rows.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

    rows.sort_by(|a, b| {
        let va = sort_value(&a.1, sort_key);
        let vb = sort_value(&b.1, sort_key);
        vb.cmp(&va)
    });

    if args.reverse {
        rows.reverse();
    }

    Ok((repo_name, rows))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_process_repo_default() {
        let args = Args::try_parse_from(["git-repostats", "-s"]).expect("Failed to parse args");
        let (repo_name, rows) =
            process_repo(&args.gitdir[0], &args).expect("Failed to process repo");

        assert_eq!(repo_name, "git-repostats");
        assert!(!rows.is_empty());

        for (author, stats) in &rows {
            assert!(!author.is_empty());
            assert!(stats.commits.is_some());
            assert!(stats.commits.unwrap() > 0);
        }

        // Verify descending order
        for window in rows.windows(2) {
            let a_commits = window[0].1.commits.unwrap_or(0);
            let b_commits = window[1].1.commits.unwrap_or(0);
            assert!(a_commits >= b_commits);
        }
    }

    #[test]
    fn test_process_repo_with_all_metrics() {
        let args = Args::try_parse_from([
            "git-repostats",
            "-s",
            "--metrics",
            "commits",
            "files",
            "history",
            "current",
            "all",
        ])
        .expect("Failed to parse args");

        let (repo_name, rows) =
            process_repo(&args.gitdir[0], &args).expect("Failed to process repo");

        assert_eq!(repo_name, "git-repostats");
        assert!(!rows.is_empty());

        for (_, stats) in &rows {
            assert!(stats.commits.is_some());
            assert!(stats.files.is_some());
            assert!(stats.surviving.is_some());
            assert!(stats.ins.is_some());
            assert!(stats.del.is_some());
            assert!(stats.net.is_some());
            assert!(stats.churn.is_some());

            let ins = stats.ins.unwrap();
            let del = stats.del.unwrap();
            let churn = stats.churn.unwrap();
            let net = stats.net.unwrap();

            assert_eq!(churn, ins + del);
            assert_eq!(net, ins as isize - del as isize);
        }
    }

    #[test]
    fn test_process_repo_reverse_sort() {
        let args = Args::try_parse_from(["git-repostats", "-s", "--reverse"])
            .expect("Failed to parse args");
        let (_, rows) = process_repo(&args.gitdir[0], &args).expect("Failed to process repo");

        assert!(!rows.is_empty());

        // Verify ascending order
        for window in rows.windows(2) {
            let a_commits = window[0].1.commits.unwrap_or(0);
            let b_commits = window[1].1.commits.unwrap_or(0);
            assert!(a_commits <= b_commits);
        }
    }

    #[test]
    fn test_process_repo_sort_by_surviving() {
        let args = Args::try_parse_from([
            "git-repostats",
            "-s",
            "--metrics",
            "current",
            "--sort",
            "surviving",
        ])
        .expect("Failed to parse args");

        let (_, rows) = process_repo(&args.gitdir[0], &args).expect("Failed to process repo");
        assert!(!rows.is_empty());

        for window in rows.windows(2) {
            let a_surviving = window[0].1.surviving.unwrap_or(0);
            let b_surviving = window[1].1.surviving.unwrap_or(0);
            assert!(a_surviving >= b_surviving);
        }
    }

    #[test]
    fn test_process_repo_non_silent_progress() {
        let args = Args::try_parse_from(["git-repostats"]).expect("Failed to parse args");
        assert!(!args.silent_progress);

        let (repo_name, rows) =
            process_repo(&args.gitdir[0], &args).expect("Failed to process repo");
        assert_eq!(repo_name, "git-repostats");
        assert!(!rows.is_empty());
    }

    #[test]
    fn test_process_repo_invalid_branch() {
        let args = Args::try_parse_from([
            "git-repostats",
            "-s",
            "--branch",
            "non_existent_branch_xyz123",
        ])
        .expect("Failed to parse args");

        let res = process_repo(&args.gitdir[0], &args);
        assert!(res.is_err());
    }
}
