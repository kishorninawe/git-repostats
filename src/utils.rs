use console::style;
use num_format::{Locale, ToFormattedString};
use std::{
    collections::HashSet,
    fs::File,
    io,
    path::{PathBuf, absolute},
    process::Command,
};

use crate::{
    cli::{Args, Limit, Metrics, SortKey as Sort},
    stats::AuthorStats,
};

/// Validate that a path:
/// - exists
/// - is a directory
/// - is a git repository
pub fn validate_git_dir(path: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(path);

    // 1. Canonicalize the path to resolve symlinks and get an absolute path
    let path = path
        .canonicalize()
        .map_err(|e| format!("invalid git directory '{}': {}", path.display(), e))?;

    // 2. Exists
    if !path.exists() {
        return Err(format!("path does not exist: {}", path.display()));
    }

    // 3. Is directory
    if !path.is_dir() {
        return Err(format!("path is not a directory: {}", path.display()));
    }

    // 4. Is git repository (.git directory or file)
    let git_dir = path.join(".git");
    if !git_dir.exists() {
        return Err(format!(
            "not a git repository (missing .git): {}",
            path.display()
        ));
    }

    // 5. Is valid git repository
    check_output(&[
        "git",
        "-C",
        path.to_string_lossy().as_ref(),
        "rev-parse",
        "--git-dir",
    ])
    .map_err(|e| {
        format!(
            "not a valid git repository: {} (Error: {})",
            path.display(),
            e
        )
    })?;

    Ok(path)
}

pub fn validate_ignore_revs_file(file: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(file);

    let path = absolute(&path).map_err(|e| format!("cannot resolve {}: {e}", path.display()))?;

    if !path.is_file() {
        return Err(format!("not a file: {}", path.display()));
    }

    File::open(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;

    Ok(path)
}

pub fn parse_metrics(metrics: &[Metrics]) -> Vec<Metrics> {
    let unique: HashSet<_> = metrics.iter().copied().collect();

    if unique.contains(&Metrics::All) {
        return vec![
            Metrics::Commits,
            Metrics::Files,
            Metrics::History,
            Metrics::Current,
        ];
    }

    unique.into_iter().collect()
}

pub fn parse_limit(s: &str) -> Result<Limit, String> {
    match s {
        "all" => Ok(Limit::All),
        _ => {
            let n: usize = s
                .parse()
                .map_err(|_| "limit must be a positive number or `all`")?;
            if n == 0 {
                Err("limit must be greater than 0".into())
            } else {
                Ok(Limit::Count(n))
            }
        }
    }
}

pub fn push_opt_arg(cmd: &mut Command, flag: &'static str, value: Option<&str>) {
    if let Some(v) = value {
        cmd.arg(flag).arg(v);
    }
}

pub fn push_flag(cmd: &mut Command, flag: &'static str, enabled: bool) {
    if enabled {
        cmd.arg(flag);
    }
}

pub fn check_output(argv: &[&str]) -> io::Result<String> {
    let (program, args) = argv
        .split_first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "empty command"))?;

    let output = Command::new(program).args(args).output()?;

    if !output.status.success() {
        return Err(io::Error::other(
            String::from_utf8_lossy(&output.stderr).trim(),
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn is_sort_valid(sort: Sort, args: &Args) -> bool {
    args.metrics.iter().any(|m| match m {
        Metrics::Commits => matches!(sort, Sort::Commits),
        Metrics::Files => matches!(sort, Sort::Files),
        Metrics::History => matches!(
            sort,
            Sort::Insertions | Sort::Deletions | Sort::Net | Sort::Churn
        ),
        Metrics::Current => matches!(sort, Sort::Surviving),
        Metrics::All => matches!(
            sort,
            Sort::Commits
                | Sort::Files
                | Sort::Insertions
                | Sort::Deletions
                | Sort::Net
                | Sort::Churn
                | Sort::Surviving
        ),
    })
}

pub fn get_sort_key(args: &Args) -> Sort {
    // user provided sort
    if let Some(s) = args.sort {
        if is_sort_valid(s, args) {
            return s;
        } else {
            eprintln!(
                "{} sort key {:?} is not valid for the selected metrics. Using default sort key.",
                style("Warning:").yellow().bold(),
                s
            );
        }
    }

    // fallback based on selected metrics
    for m in &args.metrics {
        let default = match m {
            Metrics::Commits => Some(Sort::Commits),
            Metrics::Files => Some(Sort::Files),
            Metrics::History => Some(Sort::Churn),
            Metrics::Current => Some(Sort::Surviving),
            Metrics::All => Some(Sort::Surviving),
        };

        if let Some(sort) = default {
            return sort;
        }
    }

    // final safe fallback (should never really hit)
    Sort::Commits
}

fn opt_usize(v: Option<usize>) -> isize {
    v.map(|n| n as isize).unwrap_or(0)
}

pub fn sort_value(stats: &AuthorStats, sort: Sort) -> isize {
    match sort {
        Sort::Commits => opt_usize(stats.commits),
        Sort::Files => opt_usize(stats.files),
        Sort::Insertions => opt_usize(stats.ins),
        Sort::Deletions => opt_usize(stats.del),
        Sort::Net => stats.net.unwrap_or(0),
        Sort::Churn => opt_usize(stats.churn),
        Sort::Surviving => opt_usize(stats.surviving),
    }
}

pub fn fmt_number<T: ToFormattedString>(n: &Option<T>) -> String {
    match n {
        Some(v) => v.to_formatted_string(&Locale::en),
        None => "".to_owned(),
    }
}
