use crate::cli::{Args, Metrics, SortKey};
use crate::git::{git_blame, git_list_files, git_log, git_shortlog};
use crate::stats::AuthorStats;
use crate::utils::sort_value;
use console::style;
use indicatif::{HumanDuration, MultiProgress, ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

pub fn process_repo(dir: &str, args: &Args) -> Vec<(String, AuthorStats)> {
    let repo_name: &str = Path::new(&dir)
        .components()
        .last()
        .and_then(|c| c.as_os_str().to_str())
        .unwrap();
    let started = Instant::now();
    let m = MultiProgress::new();

    let mut result: HashMap<String, AuthorStats> = HashMap::new();
    let files = git_list_files(dir, args);

    let total_steps = (args.metrics.contains(&Metrics::Files)
        || args.metrics.contains(&Metrics::History)) as u64
        + args.metrics.contains(&Metrics::Commits) as u64
        + args
            .metrics
            .contains(&Metrics::Current)
            .then(|| files.len() as u64)
            .unwrap_or(0);

    /* Bar 1: repo-level progress (known total) */
    let repo_pb = m.add(ProgressBar::new(total_steps));
    repo_pb.enable_steady_tick(Duration::from_millis(200));
    repo_pb.set_style(
        ProgressStyle::with_template("{spinner:.white} {msg:.dim} [{elapsed_precise:.cyan} < {eta_precise:.dim}] ({pos:.green}/{len:.green})")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    repo_pb.set_message(format!("Processing {}...", repo_name));

    /* Bar 2: git command progress */
    let git_pb = m.add(ProgressBar::new_spinner());
    git_pb.enable_steady_tick(Duration::from_millis(200));
    git_pb.set_style(
        ProgressStyle::with_template("{spinner:.white} {msg:.dim}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );

    let git_log_data = args
        .metrics
        .iter()
        .any(|m| matches!(m, Metrics::Files | Metrics::History))
        .then(|| git_log(dir, args, &files, &repo_pb, &git_pb));

    for m in &args.metrics {
        match m {
            Metrics::Commits => {
                let commit_counts = git_shortlog(dir, args, &repo_pb, &git_pb);
                for (author, commits) in commit_counts {
                    let entry: &mut _ = result.entry(author).or_default();
                    entry.commits = Some(entry.commits.unwrap_or(0) + commits);
                }
            }
            Metrics::Files => {
                let history_stats = git_log_data.as_ref().unwrap();
                for (author, h) in history_stats {
                    let entry: &mut _ = result.entry(author.to_string()).or_default();
                    entry.files = Some(entry.files.unwrap_or(0) + h.files.len());
                }
            }
            Metrics::History => {
                let history_stats = git_log_data.as_ref().unwrap();
                for (author, h) in history_stats {
                    let entry = result.entry(author.to_string()).or_default();
                    entry.ins = Some(entry.ins.unwrap_or(0) + h.ins);
                    entry.del = Some(entry.del.unwrap_or(0) + h.del);
                    entry.net = Some(entry.net.unwrap_or(0) + h.net);
                    entry.churn = Some(entry.churn.unwrap_or(0) + h.churn);
                }
            }
            Metrics::Current => {
                let current_stats = git_blame(dir, args, &files, &repo_pb, &git_pb);
                for (author, c) in current_stats {
                    let entry = result.entry(author).or_default();
                    entry.surviving = Some(entry.surviving.unwrap_or(0) + c.surviving);
                }
            }
            Metrics::All => {}
        }
    }

    git_pb.finish_and_clear();
    repo_pb.finish_and_clear();
    m.println(
        format!(
            "{} {} {}",
            style("Done").dim(),
            style(repo_name).bold().dim(),
            style(format!("in {}", HumanDuration(started.elapsed()))).dim()
        )
        .to_string(),
    )
    .unwrap();
    m.clear().unwrap();

    let sort_key: SortKey = args.sort.unwrap_or_else(|| SortKey::Commits);
    let mut rows: Vec<(String, AuthorStats)> = result.into_iter().collect();

    rows.sort_by(|a, b| {
        let va = sort_value(&a.1, sort_key);
        let vb = sort_value(&b.1, sort_key);
        vb.cmp(&va)
    });

    if args.reverse {
        rows.reverse();
    }

    rows
}
